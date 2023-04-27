/// This module is a partial implementation of the Debian DebHelper core library
/// aka dh_lib. Specifically this implementation is based on the Ubuntu version
/// labelled 12.10ubuntu1 which is included in Ubuntu 20.04 LTS. I believe 12 is
/// a reference to Debian 12 "Bookworm", i.e. Ubuntu uses future Debian sources
/// and is also referred to as compat level 12 by debhelper documentation. Only
/// functionality that was needed to properly script installation of systemd
/// units, i.e. that used by the debhelper dh_instalsystemd command or rather
/// our dh_installsystemd.rs implementation of it, is included here.
///
/// # See also
///
/// Ubuntu 20.04 dh_lib sources:
/// <https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1>
///
/// Ubuntu 20.04 dh_installsystemd man page (online HTML version):
/// <http://manpages.ubuntu.com/manpages/focal/en/man1/dh_installdeb.1.html>
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::*;
use crate::util::{is_path_file, read_file_to_string};
use crate::{listener::Listener, CDResult};

/// DebHelper autoscripts are embedded in the Rust library binary.
/// The autoscripts were taken from:
///   https://git.launchpad.net/ubuntu/+source/debhelper/tree/autoscripts?h=applied/12.10ubuntu1
/// To understand which scripts are invoked when, consult:
///   https://www.debian.org/doc/debian-policy/ap-flowcharts.htm

static AUTOSCRIPTS: [(&str, &[u8]); 10] = [
    ("postinst-init-tmpfiles", include_bytes!("../autoscripts/postinst-init-tmpfiles")),
    ("postinst-systemd-dont-enable", include_bytes!("../autoscripts/postinst-systemd-dont-enable")),
    ("postinst-systemd-enable", include_bytes!("../autoscripts/postinst-systemd-enable")),
    ("postinst-systemd-restart", include_bytes!("../autoscripts/postinst-systemd-restart")),
    ("postinst-systemd-restartnostart", include_bytes!("../autoscripts/postinst-systemd-restartnostart")),
    ("postinst-systemd-start", include_bytes!("../autoscripts/postinst-systemd-start")),
    ("postrm-systemd", include_bytes!("../autoscripts/postrm-systemd")),
    ("postrm-systemd-reload-only", include_bytes!("../autoscripts/postrm-systemd-reload-only")),
    ("prerm-systemd", include_bytes!("../autoscripts/prerm-systemd")),
    ("prerm-systemd-restart", include_bytes!("../autoscripts/prerm-systemd-restart")),
];
pub(crate) type ScriptFragments = HashMap<String, Vec<u8>>;

/// Find a file in the given directory that best matches the given package,
/// filename and (optional) unit name. Enables callers to use the most specific
/// match while also falling back to a less specific match (e.g. a file to be
/// used as a default) when more specific matches are not available.
///
/// Returns one of the following, in order of most preferred first:
///
///   - `Some("<dir>/<package>.<unit_name>.<filename>")`
///   - `Some("<dir>/<package>.<filename>")`
///   - `Some("<dir>/<unit_name>.<filename>")`
///   - `Some("<dir>/<filename>")`
///   - `None`
///
/// <filename> is either a systemd unit type such as `service` or `socket`, or a
/// maintainer script name such as `postinst`.
///
/// Note: main_package should ne the first package listed in the Debian package
/// control file.
///
/// # Known limitations
///
/// The pkgfile() subroutine in the actual dh_installsystemd code is capable of
/// matching architecture and O/S specific unit files, but this implementation
/// does not support architecture or O/S specific unit files.
///
/// # References
///
/// <https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n286>
/// <https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n957>
pub(crate) fn pkgfile(dir: &Path, main_package: &str, package: &str, filename: &str, unit_name: Option<&str>) -> Option<PathBuf> {
    let mut paths_to_try = Vec::new();
    let is_main_package = main_package == package;

    // From man 1 dh_installsystemd on Ubuntu 20.04 LTS. See:
    //   http://manpages.ubuntu.com/manpages/focal/en/man1/dh_installsystemd.1.html
    // --name=name
    //     ...
    //     It changes the name that dh_installsystemd uses when it looks for
    //     maintainer provided systemd unit files as listed in the "FILES"
    //     section.  As an example, dh_installsystemd --name foo will look for
    //     debian/package.foo.service instead of debian/package.service).  These
    //     unit files are installed as name.unit-extension (in the example, it
    //     would be installed as foo.service).
    //     ...
    if let Some(str) = unit_name {
        let named_filename = format!("{str}.{filename}");
        paths_to_try.push(dir.join(format!("{package}.{named_filename}")));
        if is_main_package {
            paths_to_try.push(dir.join(named_filename));
        }
    }

    paths_to_try.push(dir.join(format!("{package}.{filename}")));
    if is_main_package {
        paths_to_try.push(dir.join(filename));
    }

    paths_to_try.into_iter().find(|p| is_path_file(p))
}

/// Get the bytes for the specified filename whose contents were embedded in our
/// binary by the rust-embed crate. See #[derive(RustEmbed)] above, decode them
/// as UTF-8 and return as an owned copy of the resulting String. Also appends
/// a trailing newline '\n' if missing.
pub(crate) fn get_embedded_autoscript(snippet_filename: &str) -> String {
    let mut snippet: Option<String> = None;

    // load from test data if defined
    if cfg!(test) {
        let path = Path::new(snippet_filename);
        if is_path_file(path) {
            snippet = read_file_to_string(path).ok();
        }
    }

    // else load from embedded strings
    let mut snippet = snippet.unwrap_or_else(|| {
        let (_, snippet_bytes) = AUTOSCRIPTS.iter().find(|(s, _)| *s == snippet_filename)
            .unwrap_or_else(|| panic!("Unknown autoscript '{}'", snippet_filename));

        // convert to string
        String::from_utf8_lossy(snippet_bytes).into_owned()
    });

    // normalize
    if !snippet.ends_with('\n') {
        snippet.push('\n');
    }

    // return
    snippet
}

/// Build up one or more shell script fragments for a given maintainer script
/// for a debian package in preparation for writing them into or as complete
/// maintainer scripts in `apply()`, pulling fragments from a "library" of
/// so-called "autoscripts".
///
/// Takes a map of values to search and replace in the selected "autoscript"
/// fragment such as a systemd unit name placeholder and value.
///
/// # Cargo Deb specific behaviour
///
/// The autoscripts are sourced from within the binary via the rust_embed crate.
///
/// Results are stored as updated or new entries in the `ScriptFragments` map,
/// rather than being written to temporary files on disk.
///
/// # Known limitations
///
/// Arbitrary sed command based file editing is not supported.
///
/// # References
///
/// <https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n1135>
pub(crate) fn autoscript(
    scripts: &mut ScriptFragments,
    package: &str,
    script: &str,
    snippet_filename: &str,
    replacements: &HashMap<&str, String>,
    service_order: bool,
    listener: &dyn Listener,
) -> CDResult<()> {
    let bin_name = std::env::current_exe().unwrap();
    let bin_name = bin_name.file_name().unwrap();
    let bin_name = bin_name.to_str().unwrap();
    let outfile_ext = if service_order { "service" } else { "debhelper" };
    let outfile = format!("{package}.{script}.{outfile_ext}");

    listener.info(format!("Maintainer script {script} will be augmented with autoscript {snippet_filename}"));

    if scripts.contains_key(&outfile) && (script == "postrm" || script == "prerm") {
        if !replacements.is_empty() {
            let existing_text = std::str::from_utf8(scripts.get(&outfile).unwrap())?;

            // prepend new text to existing script fragment
            let new_text = [
                &format!("# Automatically added by {bin_name}\n"),
                &autoscript_sed(snippet_filename, replacements),
                "# End automatically added section\n",
                existing_text,
            ].concat();
            scripts.insert(outfile, new_text.into());
        } else {
            // We don't support sed commands yet.
            unimplemented!();
        }
    } else if !replacements.is_empty() {
        // append to existing script fragment (if any)
        let new_text = [
            std::str::from_utf8(scripts.get(&outfile).unwrap_or(&Vec::new()))?,
            &format!("# Automatically added by {bin_name}\n"),
            &autoscript_sed(snippet_filename, replacements),
            "# End automatically added section\n",
        ].concat();
        scripts.insert(outfile, new_text.into());
    } else {
        // We don't support sed commands yet.
        unimplemented!();
    }

    Ok(())
}

/// Search and replace a collection of key => value pairs in the given file and
/// return the resulting text as a String.
///
/// # Known limitations
///
/// Keys are replaced in arbitrary order, not in reverse sorted order. See:
///   https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n1214
///
/// # References
///
/// <https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n1203>
fn autoscript_sed(snippet_filename: &str, replacements: &HashMap<&str, String>) -> String {
    let mut snippet = get_embedded_autoscript(snippet_filename);

    for (from, to) in replacements {
        snippet = snippet.replace(&format!("#{from}#"), to);
    }

    snippet
}

/// Copy the merged autoscript fragments to the final maintainer script, either
/// at the point where the user placed a #DEBHELPER# token to indicate where
/// they should be inserted, or by adding a shebang header to make the fragments
/// into a complete shell script.
///
/// # Cargo Deb specific behaviour
///
/// Results are stored as updated or new entries in the `ScriptFragments` map,
/// rather than being written to temporary files on disk.
///
/// # Known limitations
///
/// Only the #DEBHELPER# token is replaced. Is that enough? See:
///   https://www.man7.org/linux/man-pages/man1/dh_installdeb.1.html#SUBSTITUTION_IN_MAINTAINER_SCRIPTS
///
/// # References
///
/// <https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n2161>
fn debhelper_script_subst(user_scripts_dir: &Path, scripts: &mut ScriptFragments, package: &str, script: &str, unit_name: Option<&str>,
    listener: &dyn Listener) -> CDResult<()>
{
    let user_file = pkgfile(user_scripts_dir, package, package, script, unit_name);
    let mut generated_scripts: Vec<String> = vec![
        format!("{package}.{script}.debhelper"),
        format!("{package}.{script}.service"),
    ];

    if let "prerm" | "postrm" = script {
        generated_scripts.reverse();
    }

    // merge the generated scripts if they exist into the user script
    let mut generated_text = String::new();
    for generated_file_name in generated_scripts.iter() {
        if let Some(contents) = scripts.get(generated_file_name) {
            generated_text.push_str(std::str::from_utf8(contents)?);
        }
    }

    if let Some(user_file_path) = user_file {
        listener.info(format!("Augmenting maintainer script {}", user_file_path.display()));

        // merge the generated scripts if they exist into the user script
        // if no generated script exists, we still need to remove #DEBHELPER# if
        // present otherwise the script will be syntactically invalid
        let user_text = read_file_to_string(&user_file_path)?;
        let new_text = user_text.replace("#DEBHELPER#", &generated_text);
        if new_text == user_text {
            return Err(CargoDebError::DebHelperReplaceFailed(user_file_path));
        }
        scripts.insert(script.into(), new_text.into());
    } else if !generated_text.is_empty() {
        listener.info(format!("Generating maintainer script {}", script));

        // give it a shebang header and rename it
        let mut new_text = String::new();
        new_text.push_str("#!/bin/sh\n");
        new_text.push_str("set -e\n");
        new_text.push_str(&generated_text);

        scripts.insert(script.into(), new_text.into());
    }

    Ok(())
}

/// Generate final maintainer scripts by merging the autoscripts that have been
/// collected in the `ScriptFragments` map  with the maintainer scripts
/// on disk supplied by the user.
///
/// See: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installdeb?h=applied/12.10ubuntu1#n300
pub(crate) fn apply(user_scripts_dir: &Path, scripts: &mut ScriptFragments, package: &str, unit_name: Option<&str>,
    listener: &dyn Listener) -> CDResult<()>
{
    for script in &["postinst", "preinst", "prerm", "postrm"] {
        // note: we don't support custom defines thus we don't have the final
        // 'package_subst' argument to debhelper_script_subst().
        debhelper_script_subst(user_scripts_dir, scripts, package, script, unit_name, listener)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::tests::{add_test_fs_paths, set_test_fs_path_content};
    use rstest::*;

    // helper conversion
    // create a new type to work around error "only traits defined in
    // the current crate can be implemented for arbitrary types"
    #[derive(Debug)]
    struct LocalOptionPathBuf(Option<PathBuf>);
    // Implement <&str> == <LocalOptionPathBuf> comparisons
    impl PartialEq<LocalOptionPathBuf> for &str {
        fn eq(&self, other: &LocalOptionPathBuf) -> bool {
            Some(Path::new(self).to_path_buf()) == other.0
        }
    }
    // Implement <LocalOptionPathBuf> == <&str> comparisons
    impl PartialEq<&str> for LocalOptionPathBuf {
        fn eq(&self, other: &&str) -> bool {
            self.0 == Some(Path::new(*other).to_path_buf())
        }
    }

    #[test]
    fn pkgfile_finds_most_specific_match_with_pkg_unit_file() {
        let _g = add_test_fs_paths(&[
            "/parent/dir/postinst",
            "/parent/dir/myunit.postinst",
            "/parent/dir/mypkg.postinst",
            "/parent/dir/mypkg.myunit.postinst",
            "/parent/dir/nested/mypkg.myunit.postinst",
            "/parent/mypkg.myunit.postinst",
        ]);

        let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "mypkg", "postinst", Some("myunit"));
        assert_eq!("/parent/dir/mypkg.myunit.postinst", LocalOptionPathBuf(r));

        let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "mypkg", "postinst", None);
        assert_eq!("/parent/dir/mypkg.postinst", LocalOptionPathBuf(r));
    }

    #[test]
    fn pkgfile_finds_most_specific_match_without_unit_file() {
        let _g = add_test_fs_paths(&["/parent/dir/postinst", "/parent/dir/mypkg.postinst"]);

        let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "mypkg", "postinst", Some("myunit"));
        assert_eq!("/parent/dir/mypkg.postinst", LocalOptionPathBuf(r));

        let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "mypkg", "postinst", None);
        assert_eq!("/parent/dir/mypkg.postinst", LocalOptionPathBuf(r));
    }

    #[test]
    fn pkgfile_finds_most_specific_match_without_pkg_file() {
        let _g = add_test_fs_paths(&["/parent/dir/postinst", "/parent/dir/myunit.postinst"]);

        let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "mypkg", "postinst", Some("myunit"));
        assert_eq!("/parent/dir/myunit.postinst", LocalOptionPathBuf(r));

        let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "mypkg", "postinst", None);
        assert_eq!("/parent/dir/postinst", LocalOptionPathBuf(r));
    }

    #[test]
    fn pkgfile_finds_a_fallback_match() {
        let _g = add_test_fs_paths(&[
            "/parent/dir/postinst",
            "/parent/dir/myunit.postinst",
            "/parent/dir/mypkg.postinst",
            "/parent/dir/mypkg.myunit.postinst",
            "/parent/dir/nested/mypkg.myunit.postinst",
            "/parent/mypkg.myunit.postinst",
        ]);

        let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "mypkg", "postinst", Some("wrongunit"));
        assert_eq!("/parent/dir/mypkg.postinst", LocalOptionPathBuf(r));

        let r = pkgfile(Path::new("/parent/dir/"), "wrongpkg", "wrongpkg", "postinst", None);
        assert_eq!("/parent/dir/postinst", LocalOptionPathBuf(r));
    }

    #[test]
    fn pkgfile_fails_to_find_a_match() {
        let _g = add_test_fs_paths(&[
            "/parent/dir/postinst",
            "/parent/dir/myunit.postinst",
            "/parent/dir/mypkg.postinst",
            "/parent/dir/mypkg.myunit.postinst",
            "/parent/dir/nested/mypkg.myunit.postinst",
            "/parent/mypkg.myunit.postinst",
        ]);

        let r = pkgfile(Path::new("/parent/dir/"), "mypkg", "mypkg", "wrongfile", None);
        assert_eq!(None, r);

        let r = pkgfile(Path::new("/wrong/dir/"), "mypkg", "mypkg", "postinst", None);
        assert_eq!(None, r);
    }

    fn autoscript_test_wrapper(pkg: &str, script: &str, snippet: &str, unit: &str, scripts: Option<ScriptFragments>) -> ScriptFragments {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());
        let mut scripts = scripts.unwrap_or_default();
        let replacements = map! { "UNITFILES" => unit.to_owned() };
        autoscript(&mut scripts, pkg, script, snippet, &replacements, false, &mock_listener).unwrap();
        scripts
    }

    #[test]
    #[should_panic(expected = "Unknown autoscript 'idontexist'")]
    fn autoscript_panics_with_unknown_autoscript() {
        autoscript_test_wrapper("mypkg", "somescript", "idontexist", "dummyunit", None);
    }

    #[test]
    #[should_panic(expected = "not implemented")]
    fn autoscript_panics_in_sed_mode() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());
        let mut scripts = ScriptFragments::new();

        // sed mode is when no search -> replacement pairs are defined
        let sed_mode = &HashMap::new();

        autoscript(&mut scripts, "mypkg", "somescript", "idontexist", sed_mode, false, &mock_listener).unwrap();
    }

    #[test]
    fn autoscript_check_embedded_files() {
        let mut actual_scripts: Vec<_> = AUTOSCRIPTS.iter().map(|(name, _)| *name).collect();
        actual_scripts.sort();

        let expected_scripts = vec![
            "postinst-init-tmpfiles",
            "postinst-systemd-dont-enable",
            "postinst-systemd-enable",
            "postinst-systemd-restart",
            "postinst-systemd-restartnostart",
            "postinst-systemd-start",
            "postrm-systemd",
            "postrm-systemd-reload-only",
            "prerm-systemd",
            "prerm-systemd-restart",
        ];

        assert_eq!(expected_scripts, actual_scripts);
    }

    #[test]
    fn autoscript_sanity_check_all_embedded_autoscripts() {
        for (autoscript_filename, _) in AUTOSCRIPTS.iter() {
            autoscript_test_wrapper("mypkg", "somescript", autoscript_filename, "dummyunit", None);
        }
    }

    #[rstest(maintainer_script, prepend,
        case::prerm("prerm", true),
        case::preinst("preinst", false),
        case::postinst("postinst", false),
        case::postrm("postrm", true),
    )]
    fn autoscript_detailed_check(maintainer_script: &str, prepend: bool) {
        let autoscript_name = "postrm-systemd";

        // Populate an autoscript template and add the result to a
        // collection of scripts and return it to us.
        let scripts = autoscript_test_wrapper("mypkg", maintainer_script, autoscript_name, "dummyunit", None);

        // Expect autoscript() to have created one temporary script
        // fragment called <package>.<script>.debhelper.
        assert_eq!(1, scripts.len());

        let expected_created_name = &format!("mypkg.{maintainer_script}.debhelper");
        let (created_name, created_bytes) = scripts.iter().next().unwrap();

        // Verify the created script filename key
        assert_eq!(expected_created_name, created_name);

        // Verify the created script contents. It should have two lines
        // more than the autoscript fragment it was based on, like so:
        //   # Automatically added by ...
        //   <autoscript fragment lines with placeholders replaced>
        //   # End automatically added section
        let autoscript_text = get_embedded_autoscript(autoscript_name);
        let autoscript_line_count = autoscript_text.lines().count();
        let created_text = std::str::from_utf8(created_bytes).unwrap();
        let created_line_count = created_text.lines().count();
        assert_eq!(autoscript_line_count + 2, created_line_count);

        // Verify the content of the added comment lines
        let mut lines = created_text.lines();
        assert!(lines.next().unwrap().starts_with("# Automatically added by"));
        assert_eq!(lines.nth_back(0).unwrap(), "# End automatically added section");

        // Check that the autoscript fragment lines were properly copied
        // into the created script complete with expected substitutions
        let expected_autoscript_text1 = autoscript_text.replace("#UNITFILES#", "dummyunit");
        let expected_autoscript_text1 = expected_autoscript_text1.trim_end();
        let start1 = 1;
        let end1 = start1 + autoscript_line_count;
        let created_autoscript_text1 = created_text.lines().collect::<Vec<&str>>()[start1..end1].join("\n");
        assert_ne!(expected_autoscript_text1, autoscript_text);
        assert_eq!(expected_autoscript_text1, created_autoscript_text1);

        // Process the same autoscript again but use a different unit
        // name so that we can see if the autoscript template was again
        // populated but this time with the different value, and pass in
        // the existing set of created scripts to check how it gets
        // modified.
        let scripts = autoscript_test_wrapper("mypkg", maintainer_script, autoscript_name, "otherunit", Some(scripts));

        // The number and name of the output scripts should remain the same
        assert_eq!(1, scripts.len());
        let (created_name, created_bytes) = scripts.iter().next().unwrap();
        assert_eq!(expected_created_name, created_name);

        // The line structure should now contain two injected blocks
        let created_text = std::str::from_utf8(created_bytes).unwrap();
        let created_line_count = created_text.lines().count();
        assert_eq!((autoscript_line_count + 2) * 2, created_line_count);

        let mut lines = created_text.lines();
        assert!(lines.next().unwrap().starts_with("# Automatically added by"));
        assert_eq!(lines.nth_back(0).unwrap(), "# End automatically added section");

        // The content should be different
        let expected_autoscript_text2 = autoscript_text.replace("#UNITFILES#", "otherunit");
        let expected_autoscript_text2 = expected_autoscript_text2.trim_end();
        let start2 = end1 + 2;
        let end2 = start2 + autoscript_line_count;
        let created_autoscript_text1 = created_text.lines().collect::<Vec<&str>>()[start1..end1].join("\n");
        let created_autoscript_text2 = created_text.lines().collect::<Vec<&str>>()[start2..end2].join("\n");
        assert_ne!(expected_autoscript_text1, autoscript_text);
        assert_ne!(expected_autoscript_text2, autoscript_text);

        if prepend {
            assert_eq!(expected_autoscript_text1, created_autoscript_text2);
            assert_eq!(expected_autoscript_text2, created_autoscript_text1);
        } else {
            assert_eq!(expected_autoscript_text1, created_autoscript_text1);
            assert_eq!(expected_autoscript_text2, created_autoscript_text2);
        }
    }

    #[test]
    fn autoscript_check_service_order() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().return_const(());
        let replacements = map! { "UNITFILES" => "someunit".to_owned() };

        let in_out = vec![(false, "debhelper"), (true, "service")];

        for (service_order, expected_ext) in in_out.into_iter() {
            let mut scripts = ScriptFragments::new();
            autoscript(&mut scripts, "mypkg", "prerm", "postrm-systemd", &replacements, service_order, &mock_listener).unwrap();

            assert_eq!(1, scripts.len());

            let expected_path = &format!("mypkg.prerm.{expected_ext}");
            let actual_path = scripts.keys().next().unwrap();
            assert_eq!(expected_path, actual_path);
        }
    }

    #[fixture]
    fn empty_user_file() -> String { "".to_owned() }

    #[fixture]
    fn invalid_user_file() -> String { "some content".to_owned() }

    #[fixture]
    fn valid_user_file() -> String { "some #DEBHELPER# content".to_owned() }

    #[test]
    fn debhelper_script_subst_with_no_matching_files() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());

        let mut scripts = ScriptFragments::new();

        assert_eq!(0, scripts.len());
        debhelper_script_subst(Path::new(""), &mut scripts, "mypkg", "myscript", None, &mock_listener).unwrap();
        assert_eq!(0, scripts.len());
    }

    #[rstest]
    #[should_panic(expected = "Test failed as expected")]
    fn debhelper_script_subst_errs_if_user_file_lacks_token(invalid_user_file: String) {
        let _g = add_test_fs_paths(&[]);
        set_test_fs_path_content("myscript", invalid_user_file);

        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());

        let mut scripts = ScriptFragments::new();

        match debhelper_script_subst(Path::new(""), &mut scripts, "mypkg", "myscript", None, &mock_listener) {
            Ok(_) => (),
            Err(CargoDebError::DebHelperReplaceFailed(_)) => panic!("Test failed as expected"),
            Err(err) => panic!("Unexpected error {:?}", err),
        }
    }

    #[rstest]
    #[test]
    fn debhelper_script_subst_with_user_file_only(valid_user_file: String) {
        let _g = add_test_fs_paths(&[]);
        set_test_fs_path_content("myscript", valid_user_file);

        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());

        let mut scripts = ScriptFragments::new();

        assert_eq!(0, scripts.len());
        debhelper_script_subst(Path::new(""), &mut scripts, "mypkg", "myscript", None, &mock_listener).unwrap();
        assert_eq!(1, scripts.len());
        assert!(scripts.contains_key("myscript"));
    }

    fn script_to_string(scripts: &ScriptFragments, script: &str) -> String {
        String::from_utf8(scripts.get(script).unwrap().to_vec()).unwrap()
    }

    #[test]
    fn debhelper_script_subst_with_generated_file_only() {
        let _g = add_test_fs_paths(&[]);
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());

        let mut scripts = ScriptFragments::new();
        scripts.insert("mypkg.myscript.debhelper".to_owned(), "injected".as_bytes().to_vec());

        assert_eq!(1, scripts.len());
        debhelper_script_subst(Path::new(""), &mut scripts, "mypkg", "myscript", None, &mock_listener).unwrap();
        assert_eq!(2, scripts.len());
        assert!(scripts.contains_key("mypkg.myscript.debhelper"));
        assert!(scripts.contains_key("myscript"));

        assert_eq!(script_to_string(&scripts, "mypkg.myscript.debhelper"), "injected");
        assert_eq!(script_to_string(&scripts, "myscript"), "#!/bin/sh\nset -e\ninjected");
    }

    #[rstest]
    #[test]
    fn debhelper_script_subst_with_user_and_generated_file(valid_user_file: String) {
        let _g = add_test_fs_paths(&[]);
        set_test_fs_path_content("myscript", valid_user_file);

        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());

        let mut scripts = ScriptFragments::new();
        scripts.insert("mypkg.myscript.debhelper".to_owned(), "injected".as_bytes().to_vec());

        assert_eq!(1, scripts.len());
        debhelper_script_subst(Path::new(""), &mut scripts, "mypkg", "myscript", None, &mock_listener).unwrap();
        assert_eq!(2, scripts.len());
        assert!(scripts.contains_key("mypkg.myscript.debhelper"));
        assert!(scripts.contains_key("myscript"));

        assert_eq!(script_to_string(&scripts, "mypkg.myscript.debhelper"), "injected");
        assert_eq!(script_to_string(&scripts, "myscript"), "some injected content");
    }

    #[rstest(maintainer_script, service_order,
        case("preinst", false),
        case("prerm", true),
        case("postinst", false),
        case("postrm", true),
    )]
    #[test]
    fn debhelper_script_subst_with_user_and_generated_files(
        valid_user_file: String,
        maintainer_script: &'static str,
        service_order: bool,
    ) {
        let _g = add_test_fs_paths(&[]);
        set_test_fs_path_content(maintainer_script, valid_user_file);

        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());

        let mut scripts = ScriptFragments::new();
        scripts.insert(format!("mypkg.{maintainer_script}.debhelper"), "first".as_bytes().to_vec());
        scripts.insert(format!("mypkg.{maintainer_script}.service"), "second".as_bytes().to_vec());

        assert_eq!(2, scripts.len());
        debhelper_script_subst(Path::new(""), &mut scripts, "mypkg", maintainer_script, None, &mock_listener).unwrap();
        assert_eq!(3, scripts.len());
        assert!(scripts.contains_key(&format!("mypkg.{maintainer_script}.debhelper")));
        assert!(scripts.contains_key(&format!("mypkg.{maintainer_script}.service")));
        assert!(scripts.contains_key(maintainer_script));

        assert_eq!(script_to_string(&scripts, &format!("mypkg.{maintainer_script}.debhelper")), "first");
        assert_eq!(script_to_string(&scripts, &format!("mypkg.{maintainer_script}.service")), "second");
        if service_order {
            assert_eq!(script_to_string(&scripts, maintainer_script), "some secondfirst content");
        } else {
            assert_eq!(script_to_string(&scripts, maintainer_script), "some firstsecond content");
        }
    }

    #[rstest(error,
        case::invalid_input("InvalidInput"),
        case::interrupted("Interrupted"),
        case::permission_denied("PermissionDenied"),
        case::not_found("NotFound"),
        case::other("Other")
    )]
    #[test]
    fn debhelper_script_subst_with_user_file_access_error(error: &str) {
        let _g = add_test_fs_paths(&[]);
        set_test_fs_path_content("myscript", format!("error:{error}"));

        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());

        let mut scripts = ScriptFragments::new();

        assert_eq!(0, scripts.len());
        let result = debhelper_script_subst(Path::new(""), &mut scripts, "mypkg", "myscript", None, &mock_listener);

        assert!(matches!(result, Err(CargoDebError::Io(_))));
        if let CargoDebError::Io(err) = result.unwrap_err() {
            assert_eq!(error, std::fmt::format(std::format_args!("{:?}", err.kind())));
        } else {
            unreachable!()
        }
    }

    #[test]
    fn apply_with_no_matching_files() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());
        apply(Path::new(""), &mut ScriptFragments::new(), "mypkg", None, &mock_listener).unwrap();
    }

    #[rstest]
    #[test]
    fn apply_with_valid_user_files(valid_user_file: String) {
        let _g = add_test_fs_paths(&[]);
        let scripts = &["postinst", "preinst", "prerm", "postrm"];

        for script in scripts {
            set_test_fs_path_content(script, valid_user_file.clone());
        }

        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(scripts.len()).return_const(());

        apply(Path::new(""), &mut ScriptFragments::new(), "mypkg", None, &mock_listener).unwrap();
    }
}
