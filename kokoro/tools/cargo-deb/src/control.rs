use crate::dh_installsystemd;
use crate::dh_lib;
use crate::error::*;
use crate::listener::Listener;
use crate::manifest::Config;
use crate::pathbytes::*;
use crate::tararchive::Archive;
use crate::util::{is_path_file, read_file_to_bytes};
use crate::wordsplit::WordSplit;
use dh_lib::ScriptFragments;
use md5::Digest;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct ControlArchiveBuilder<'l, W: Write> {
    archive: Archive<W>,
    listener: &'l dyn Listener,
}

impl<'l, W: Write> ControlArchiveBuilder<'l, W> {
    pub fn new(dest: W, time: u64, listener: &'l dyn Listener) -> Self {
        Self { archive: Archive::new(dest, time), listener }
    }

    /// Generates an uncompressed tar archive with `control`, `md5sums`, and others
    pub fn generate_archive(&mut self, options: &Config) -> CDResult<()> {
        self.generate_control(options)?;
        if let Some(ref files) = options.conf_files {
            self.generate_conf_files(files)?;
        }
        self.generate_scripts(options)?;
        if let Some(ref file) = options.triggers_file {
            let triggers_file = &options.package_manifest_dir.as_path().join(file);
            if !triggers_file.exists() {
                return Err(CargoDebError::AssetFileNotFound(file.to_path_buf()));
            }
            self.generate_triggers_file(triggers_file)?;
        }
        Ok(())
    }

    pub fn finish(self) -> CDResult<W> {
        Ok(self.archive.into_inner()?)
    }

    /// Append Debian maintainer script files (control, preinst, postinst, prerm,
    /// postrm and templates) present in the `maintainer_scripts` path to the
    /// archive, if `maintainer_scripts` is configured.
    ///
    /// Additionally, when `systemd_units` is configured, shell script fragments
    /// "for enabling, disabling, starting, stopping and restarting systemd unit
    /// files" (quoting man 1 dh_installsystemd) will replace the `#DEBHELPER#`
    /// token in the provided maintainer scripts.
    ///
    /// If a shell fragment cannot be inserted because the target script is missing
    /// then the entire script will be generated and appended to the archive.
    ///
    /// # Requirements
    ///
    /// When `systemd_units` is configured, user supplied `maintainer_scripts` must
    /// contain a `#DEBHELPER#` token at the point where shell script fragments
    /// should be inserted.
    fn generate_scripts(&mut self, option: &Config) -> CDResult<()> {
        if let Some(ref maintainer_scripts_dir) = option.maintainer_scripts {
            let maintainer_scripts_dir = option.package_manifest_dir.as_path().join(maintainer_scripts_dir);
            let mut scripts = ScriptFragments::with_capacity(0);

            if let Some(systemd_units_config_vec) = &option.systemd_units {
                for systemd_units_config in systemd_units_config_vec {
                    // Select and populate autoscript templates relevant to the unit
                    // file(s) in this package and the configuration settings chosen.
                    scripts = dh_installsystemd::generate(
                        &option.name,
                        &option.assets.resolved,
                        &dh_installsystemd::Options::from(systemd_units_config),
                        self.listener,
                    )?;

                    // Get Option<&str> from Option<String>
                    let unit_name = systemd_units_config.unit_name.as_deref();

                    // Replace the #DEBHELPER# token in the users maintainer scripts
                    // and/or generate maintainer scripts from scratch as needed.
                    dh_lib::apply(
                        &maintainer_scripts_dir,
                        &mut scripts,
                        &option.name,
                        unit_name,
                        self.listener)?;
                }
            }

            // Add maintainer scripts to the archive, either those supplied by the
            // user or if available prefer modified versions generated above.
            for name in &["config", "preinst", "postinst", "prerm", "postrm", "templates"] {
                let mut script = scripts.remove(&name.to_string());

                if script.is_none() {
                    let script_path = maintainer_scripts_dir.join(name);
                    if is_path_file(&script_path) {
                        script = Some(read_file_to_bytes(&script_path)?);
                    }
                }

                if let Some(contents) = script {
                    // The config, postinst, postrm, preinst, and prerm
                    // control files should use mode 0755; all other control files should use 0644.
                    // See Debian Policy Manual section 10.9
                    // and lintian tag control-file-has-bad-permissions
                    let permissions = if *name == "templates" { 0o644 } else { 0o755 };
                    self.archive.file(name, &contents, permissions)?;
                }
            }
        }

        Ok(())
    }

    /// Creates the md5sums file which contains a list of all contained files and the md5sums of each.
    pub fn generate_md5sums(&mut self, options: &Config, asset_hashes: HashMap<PathBuf, Digest>) -> CDResult<()> {
        let mut md5sums: Vec<u8> = Vec::new();

        // Collect md5sums from each asset in the archive (excludes symlinks).
        for asset in &options.assets.resolved {
            if let Some(value) = asset_hashes.get(&asset.c.target_path) {
                write!(md5sums, "{:x}", value)?;
                md5sums.write_all(b"  ")?;

                md5sums.write_all(&asset.c.target_path.as_path().as_unix_path())?;
                md5sums.write_all(&[b'\n'])?;
            }
        }

        // Write the data to the archive
        self.archive.file("./md5sums", &md5sums, 0o644)?;
        Ok(())
    }

    /// Generates the control file that obtains all the important information about the package.
    fn generate_control(&mut self, options: &Config) -> CDResult<()> {
        // Create and return the handle to the control file with write access.
        let mut control: Vec<u8> = Vec::with_capacity(1024);

        // Write all of the lines required by the control file.
        writeln!(&mut control, "Package: {}", options.deb_name)?;
        writeln!(&mut control, "Version: {}", options.deb_version)?;
        writeln!(&mut control, "Architecture: {}", options.architecture)?;
        if let Some(ref repo) = options.repository {
            if repo.starts_with("http") {
                writeln!(&mut control, "Vcs-Browser: {repo}")?;
            }
            if let Some(kind) = options.repository_type() {
                writeln!(&mut control, "Vcs-{kind}: {repo}")?;
            }
        }
        if let Some(homepage) = options.homepage.as_ref().or(options.documentation.as_ref()) {
            writeln!(&mut control, "Homepage: {homepage}")?;
        }
        if let Some(ref section) = options.section {
            writeln!(&mut control, "Section: {section}")?;
        }
        writeln!(&mut control, "Priority: {}", options.priority)?;
        writeln!(&mut control, "Maintainer: {}", options.maintainer)?;

        let installed_size = options.assets.resolved
            .iter()
            .map(|m| (m.source.file_size().unwrap_or(0)+2047)/1024) // assume 1KB of fs overhead per file
            .sum::<u64>();

        writeln!(&mut control, "Installed-Size: {installed_size}")?;

        let deps = options.get_dependencies(self.listener)?;
        if !deps.is_empty() {
            writeln!(&mut control, "Depends: {deps}")?;
        }

        if let Some(ref pre_depends) = options.pre_depends {
            let pre_depends_normalized = pre_depends.trim();

            if !pre_depends_normalized.is_empty() {
                writeln!(&mut control, "Pre-Depends: {pre_depends_normalized}")?;
            }
        }

        if let Some(ref recommends) = options.recommends {
            let recommends_normalized = recommends.trim();

            if !recommends_normalized.is_empty() {
                writeln!(&mut control, "Recommends: {recommends_normalized}")?;
            }
        }

        if let Some(ref suggests) = options.suggests {
            let suggests_normalized = suggests.trim();

            if !suggests_normalized.is_empty() {
                writeln!(&mut control, "Suggests: {suggests_normalized}")?;
            }
        }

        if let Some(ref enhances) = options.enhances {
            let enhances_normalized = enhances.trim();

            if !enhances_normalized.is_empty() {
                writeln!(&mut control, "Enhances: {enhances_normalized}")?;
            }
        }

        if let Some(ref conflicts) = options.conflicts {
            writeln!(&mut control, "Conflicts: {conflicts}")?;
        }
        if let Some(ref breaks) = options.breaks {
            writeln!(&mut control, "Breaks: {breaks}")?;
        }
        if let Some(ref replaces) = options.replaces {
            writeln!(&mut control, "Replaces: {replaces}")?;
        }
        if let Some(ref provides) = options.provides {
            writeln!(&mut control, "Provides: {provides}")?;
        }

        write!(&mut control, "Description:")?;
        for line in options.description.split_by_chars(79) {
            writeln!(&mut control, " {line}")?;
        }

        if let Some(ref desc) = options.extended_description {
            for line in desc.split_by_chars(79) {
                writeln!(&mut control, " {line}")?;
            }
        }
        control.push(10);

        // Add the control file to the tar archive.
        self.archive.file("./control", &control, 0o644)?;
        Ok(())
    }

    /// If configuration files are required, the conffiles file will be created.
    fn generate_conf_files(&mut self, files: &str) -> CDResult<()> {
        let mut data = Vec::new();
        data.write_all(files.as_bytes())?;
        data.push(b'\n');
        self.archive.file("./conffiles", &data, 0o644)?;
        Ok(())
    }

    fn generate_triggers_file(&mut self, path: &Path) -> CDResult<()> {
        if let Ok(content) = fs::read(path) {
            self.archive.file("./triggers", &content, 0o644)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // The following test suite verifies that `fn generate_scripts()` correctly
    // copies "maintainer scripts" (files with the name config, preinst, postinst,
    // prerm, postrm, and/or templates) from the `maintainer_scripts` directory
    // into the generated archive, and in the case that a systemd config is
    // provided, that a service file when present causes #DEBHELPER# placeholders
    // in the maintainer scripts to be replaced and missing maintainer scripts to
    // be generated.
    //
    // The exact details of maintainer script replacement is tested
    // in `dh_installsystemd.rs`, here we are more interested in testing that
    // `fn generate_scripts()` correctly looks for maintainer script and unit
    // script files relative to the crate root, whether processing the root crate
    // or a workspace member crate.
    //
    // This test depends on the existence of two test crates organized such that
    // one is a Cargo workspace member and the other is a root crate.
    //
    //   test-resources/
    //     testroot/         <-- root crate
    //       Cargo.toml
    //       testchild/      <-- workspace member crate
    //         Cargo.toml

    use super::*;
    use crate::listener::MockListener;
    use crate::manifest::{Asset, AssetSource, SystemdUnitsConfig, IsBuilt};
    use crate::util::tests::{add_test_fs_paths, set_test_fs_path_content};
    use std::io::prelude::Read;

    fn filename_from_path_str(path: &str) -> String {
        Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string()
    }

    fn decode_name<R>(entry: &tar::Entry<R>) -> String where R: Read {
        std::str::from_utf8(&entry.path_bytes()).unwrap().to_string()
    }

    fn decode_names<R>(ar: &mut tar::Archive<R>) -> Vec<String> where R: Read {
        ar.entries().unwrap().map(|e| decode_name(&e.unwrap())).collect()
    }

    fn extract_contents<R>(ar: &mut tar::Archive<R>) -> HashMap<String, String> where R: Read {
        let mut out = HashMap::new();
        for entry in ar.entries().unwrap() {
            let mut unwrapped = entry.unwrap();
            let name = decode_name(&unwrapped);
            let mut buf = Vec::new();
            unwrapped.read_to_end(&mut buf).unwrap();
            let content = String::from_utf8(buf).unwrap();
            out.insert(name, content);
        }
        out
    }

    #[track_caller]
    fn prepare<'l, W: Write>(dest: W, package_name: Option<&str>, mock_listener: &'l mut MockListener) -> (Config, ControlArchiveBuilder<'l, W>) {
        mock_listener.expect_info().return_const(());

        let mut config = Config::from_manifest(
            Path::new("test-resources/testroot/Cargo.toml"),
            package_name,
            None,
            None,
            None,
            None,
            None,
            mock_listener,
            "release",
        )
        .unwrap();

        // make the absolute manifest dir relative to our crate root dir
        // as the static paths we receive from the caller cannot be set
        // to the absolute path we find ourselves in at test run time, but
        // instead have to match exactly the paths looked up based on the
        // value of the manifest dir.
        config.package_manifest_dir = config.package_manifest_dir.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap().to_path_buf();

        let ar = ControlArchiveBuilder::new(dest, 0, mock_listener);

        (config, ar)
    }

    #[test]
    fn generate_scripts_does_nothing_if_maintainer_scripts_is_not_set() {
        let mut listener = MockListener::new();
        let (config, mut in_ar) = prepare(vec![], None, &mut listener);

        // supply a maintainer script as if it were available on disk
        let _g = add_test_fs_paths(&["debian/postinst"]);

        // generate scripts and store them in the given archive
        in_ar.generate_scripts(&config).unwrap();

        // finish the archive and unwrap it as a byte vector
        let archive_bytes = in_ar.finish().unwrap();

        // parse the archive bytes
        let mut out_ar = tar::Archive::new(&archive_bytes[..]);

        // compare the file names in the archive to what we expect
        let archived_file_names = decode_names(&mut out_ar);
        assert!(archived_file_names.is_empty());
    }

    #[test]
    fn generate_scripts_archives_user_supplied_maintainer_scripts_in_root_package() {
        let maintainer_script_paths = vec![
            "test-resources/testroot/debian/config",
            "test-resources/testroot/debian/preinst",
            "test-resources/testroot/debian/postinst",
            "test-resources/testroot/debian/prerm",
            "test-resources/testroot/debian/postrm",
            "test-resources/testroot/debian/templates",
        ];
        generate_scripts_for_package_without_systemd_unit(None, &maintainer_script_paths);
    }

    #[test]
    fn generate_scripts_archives_user_supplied_maintainer_scripts_in_workspace_package() {
        let maintainer_script_paths = vec![
            "test-resources/testroot/testchild/debian/config",
            "test-resources/testroot/testchild/debian/preinst",
            "test-resources/testroot/testchild/debian/postinst",
            "test-resources/testroot/testchild/debian/prerm",
            "test-resources/testroot/testchild/debian/postrm",
            "test-resources/testroot/testchild/debian/templates",
        ];
        generate_scripts_for_package_without_systemd_unit(Some("test_child"), &maintainer_script_paths);
    }

    #[track_caller]
    fn generate_scripts_for_package_without_systemd_unit(package_name: Option<&str>, maintainer_script_paths: &[&'static str]) {
        let mut listener = MockListener::new();
        let (mut config, mut in_ar) = prepare(vec![], package_name, &mut listener);

        // supply a maintainer script as if it were available on disk
        // provide file content that we can easily verify
        let mut maintainer_script_contents = Vec::new();
        for script in maintainer_script_paths.iter() {
            let content = format!("some contents: {script}");
            set_test_fs_path_content(script, content.clone());
            maintainer_script_contents.push(content);
        }

        // specify a path relative to the (root or workspace child) package
        config.maintainer_scripts.get_or_insert(PathBuf::from("debian"));

        // generate scripts and store them in the given archive
        in_ar.generate_scripts(&config).unwrap();

        // finish the archive and unwrap it as a byte vector
        let archive_bytes = in_ar.finish().unwrap();

        // parse the archive bytes
        let mut out_ar = tar::Archive::new(&archive_bytes[..]);

        // compare the file contents in the archive to what we expect
        let archived_content = extract_contents(&mut out_ar);

        assert_eq!(maintainer_script_paths.len(), archived_content.len());

        // verify that the content we supplied was faithfully archived
        for script in maintainer_script_paths.iter() {
            let expected_content = &format!("some contents: {script}");
            let filename = filename_from_path_str(script);
            let actual_content = archived_content.get(&filename).unwrap();
            assert_eq!(expected_content, actual_content);
        }
    }

    #[test]
    fn generate_scripts_augments_maintainer_scripts_for_unit_in_root_package() {
        let maintainer_scripts = vec![
            ("test-resources/testroot/debian/config", Some("dummy content")),
            ("test-resources/testroot/debian/preinst", Some("dummy content\n#DEBHELPER#")),
            ("test-resources/testroot/debian/postinst", Some("dummy content\n#DEBHELPER#")),
            ("test-resources/testroot/debian/prerm", Some("dummy content\n#DEBHELPER#")),
            ("test-resources/testroot/debian/postrm", Some("dummy content\n#DEBHELPER#")),
            ("test-resources/testroot/debian/templates", Some("dummy content")),
        ];
        generate_scripts_for_package_with_systemd_unit(None, &maintainer_scripts, "test-resources/testroot/debian/some.service");
    }

    #[test]
    fn generate_scripts_augments_maintainer_scripts_for_unit_in_workspace_package() {
        let maintainer_scripts = vec![
            ("test-resources/testroot/testchild/debian/config", Some("dummy content")),
            ("test-resources/testroot/testchild/debian/preinst", Some("dummy content\n#DEBHELPER#")),
            ("test-resources/testroot/testchild/debian/postinst", Some("dummy content\n#DEBHELPER#")),
            ("test-resources/testroot/testchild/debian/prerm", Some("dummy content\n#DEBHELPER#")),
            ("test-resources/testroot/testchild/debian/postrm", Some("dummy content\n#DEBHELPER#")),
            ("test-resources/testroot/testchild/debian/templates", Some("dummy content")),
        ];
        generate_scripts_for_package_with_systemd_unit(
            Some("test_child"),
            &maintainer_scripts,
            "test-resources/testroot/testchild/debian/some.service",
        );
    }

    #[test]
    fn generate_scripts_generates_missing_maintainer_scripts_for_unit_in_root_package() {
        let maintainer_scripts = vec![
            ("test-resources/testroot/debian/postinst", None),
            ("test-resources/testroot/debian/prerm", None),
            ("test-resources/testroot/debian/postrm", None),
        ];
        generate_scripts_for_package_with_systemd_unit(None, &maintainer_scripts, "test-resources/testroot/debian/some.service");
    }

    #[test]
    fn generate_scripts_generates_missing_maintainer_scripts_for_unit_in_workspace_package() {
        let maintainer_scripts = vec![
            ("test-resources/testroot/testchild/debian/postinst", None),
            ("test-resources/testroot/testchild/debian/prerm", None),
            ("test-resources/testroot/testchild/debian/postrm", None),
        ];
        generate_scripts_for_package_with_systemd_unit(
            Some("test_child"),
            &maintainer_scripts,
            "test-resources/testroot/testchild/debian/some.service",
        );
    }

    // `maintainer_scripts` is a collection of file system paths for which:
    //   - each file should be in the same directory
    //   - the generated archive should contain a file with each of the given filenames
    //   - if Some(...) then pretend when creating the archive that a file at that path exists with the given content
    #[track_caller]
    fn generate_scripts_for_package_with_systemd_unit(
        package_name: Option<&str>,
        maintainer_scripts: &[(&'static str, Option<&'static str>)],
        service_file: &'static str,
    ) {
        let mut listener = MockListener::new();
        let (mut config, mut in_ar) = prepare(vec![], package_name, &mut listener);

        // supply a maintainer script as if it were available on disk
        // provide file content that we can easily verify
        let mut maintainer_script_contents = Vec::new();
        for (script, content) in maintainer_scripts.iter() {
            if let Some(content) = content {
                set_test_fs_path_content(script, content.to_string());
                maintainer_script_contents.push(content);
            }
        }

        set_test_fs_path_content(service_file, "mock service file".to_string());

        // make the unit file available for systemd unit processing
        let source = AssetSource::Path(PathBuf::from(service_file));
        let target_path = PathBuf::from(format!("lib/systemd/system/{}", filename_from_path_str(service_file)));
        config.assets.resolved.push(Asset::new(source, target_path, 0o000, IsBuilt::No));

        // look in the current dir for maintainer scripts (none, but the systemd
        // unit processing will be skipped if we don't set this)
        config.maintainer_scripts.get_or_insert(PathBuf::from("debian"));

        // enable systemd unit processing
        config.systemd_units.get_or_insert(vec![SystemdUnitsConfig::default()]);

        // generate scripts and store them in the given archive
        in_ar.generate_scripts(&config).unwrap();

        // finish the archive and unwrap it as a byte vector
        let archive_bytes = in_ar.finish().unwrap();

        // check that the expected files were included in the archive
        let mut out_ar = tar::Archive::new(&archive_bytes[..]);

        let mut archived_file_names = decode_names(&mut out_ar);
        archived_file_names.sort();

        let mut expected_maintainer_scripts = maintainer_scripts
            .iter()
            .map(|(script, _)| filename_from_path_str(script))
            .collect::<Vec<String>>();
        expected_maintainer_scripts.sort();

        assert_eq!(expected_maintainer_scripts, archived_file_names);

        // check the content of the archived files for any unreplaced placeholders.
        // create a new tar wrapper around the bytes as you cannot seek the same
        // Archive more than once.
        let mut out_ar = tar::Archive::new(&archive_bytes[..]);

        let unreplaced_placeholders = out_ar
            .entries()
            .unwrap()
            .map(Result::unwrap)
            .map(|mut entry| {
                let mut v = String::new();
                entry.read_to_string(&mut v).unwrap();
                v
            })
            .any(|v| v.contains("#DEBHELPER#"));

        assert!(!unreplaced_placeholders);
    }
}
