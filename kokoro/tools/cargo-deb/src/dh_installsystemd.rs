/// This module is a partial implementation of the Debian DebHelper command
/// for properly installing systemd units as part of a .deb package install aka
/// dh_installsystemd. Specifically this implementation is based on the Ubuntu
/// version labelled 12.10ubuntu1 which is included in Ubuntu 20.04 LTS. For
/// more details on the source version see the comments in dh_lib.rs.
///
/// # See also
///
/// Ubuntu 20.04 dh_installsystemd sources:
/// <https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1>
///
/// Ubuntu 20.04 dh_installsystemd man page (online HTML version):
/// <http://manpages.ubuntu.com/manpages/focal/en/man1/dh_installsystemd.1.html>
use itertools::Itertools; // for .next_tuple()

use std::collections::{BTreeSet, HashMap};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::str;

use crate::dh_lib::*;
use crate::listener::Listener;
use crate::manifest::Asset;
use crate::util::*;
use crate::CDResult;

/// From man 1 dh_installsystemd on Ubuntu 20.04 LTS. See:
///   <http://manpages.ubuntu.com/manpages/focal/en/man1/dh_installsystemd.1.html>
/// FILES
///        debian/package.mount, debian/package.path, debian/package@.path,
///        debian/package.service, debian/package@.service,
///        debian/package.socket, debian/package@.socket, debian/package.target,
///        debian/package@.target, debian/package.timer, debian/package@.timer
///            If any of those files exists, they are installed into
///            lib/systemd/system/ in the package build directory.
///        debian/package.tmpfile
///            Only used in compat 12 or earlier.  In compat 13+, this file is
///            handled by dh_installtmpfiles(1) instead.
///            If this exists, it is installed into usr/lib/tmpfiles.d/ in the
///            package build directory. Note that the "tmpfiles.d" mechanism is
///            currently only used by systemd.
const LIB_SYSTEMD_SYSTEM_DIR: &str = "lib/systemd/system/";
const USR_LIB_TMPFILES_D_DIR: &str = "usr/lib/tmpfiles.d/";
const SYSTEMD_UNIT_FILE_INSTALL_MAPPINGS: [(&str, &str, &str); 12] = [
    ("",  "mount",   LIB_SYSTEMD_SYSTEM_DIR),
    ("",  "path",    LIB_SYSTEMD_SYSTEM_DIR),
    ("@", "path",    LIB_SYSTEMD_SYSTEM_DIR),
    ("",  "service", LIB_SYSTEMD_SYSTEM_DIR),
    ("@", "service", LIB_SYSTEMD_SYSTEM_DIR),
    ("",  "socket",  LIB_SYSTEMD_SYSTEM_DIR),
    ("@", "socket",  LIB_SYSTEMD_SYSTEM_DIR),
    ("",  "target",  LIB_SYSTEMD_SYSTEM_DIR),
    ("@", "target",  LIB_SYSTEMD_SYSTEM_DIR),
    ("",  "timer",   LIB_SYSTEMD_SYSTEM_DIR),
    ("@", "timer",   LIB_SYSTEMD_SYSTEM_DIR),
    ("",  "tmpfile", USR_LIB_TMPFILES_D_DIR),
];

#[derive(Debug, PartialEq, Eq)]
pub struct InstallRecipe {
    pub path: PathBuf,
    pub mode: u32,
}

pub type PackageUnitFiles = HashMap<PathBuf, InstallRecipe>;

/// From man 1 dh_installsystemd on Ubuntu 20.04 LTS. See:
///   http://manpages.ubuntu.com/manpages/focal/en/man1/dh_installsystemd.1.html
/// > --no-enable
/// > Disable the service(s) on purge, but do not enable them on install.
/// >
/// > Note that this option does not affect whether the services are started.  Please
/// > remember to also use --no-start if the service should not be started.
/// >
/// > --name=name
/// > This option controls several things.
/// >
/// > It changes the name that dh_installsystemd uses when it looks for maintainer provided
/// > systemd unit files as listed in the "FILES" section.  As an example, dh_installsystemd
/// > --name foo will look for debian/package.foo.service instead of
/// > debian/package.service).  These unit files are installed as name.unit-extension (in
/// > the example, it would be installed as foo.service).
/// >
/// > Furthermore, if no unit files are passed explicitly as command line arguments,
/// > dh_installsystemd will only act on unit files called name (rather than all unit files
/// > found in the package).
/// >
/// > --restart-after-upgrade
/// > Do not stop the unit file until after the package upgrade has been completed.  This is
/// > the default behaviour in compat 10.
/// >
/// > In earlier compat levels the default was to stop the unit file in the prerm, and start
/// > it again in the postinst.
/// >
/// > This can be useful for daemons that should not have a possibly long downtime during
/// > upgrade. But you should make sure that the daemon will not get confused by the package
/// > being upgraded while it's running before using this option.
/// >
/// > --no-restart-after-upgrade
/// > Undo a previous --restart-after-upgrade (or the default of compat 10).  If no other
/// > options are given, this will cause the service to be stopped in the prerm script and
/// > started again in the postinst script.
/// >
/// > -r, --no-stop-on-upgrade, --no-restart-on-upgrade
/// > Do not stop service on upgrade.
/// >
/// > --no-start
/// > Do not start the unit file after upgrades and after initial installation (the latter
/// > is only relevant for services without a corresponding init script).
/// >
/// > Note that this option does not affect whether the services are enabled.  Please
/// > remember to also use --no-enable if the services should not be enabled.
/// >
/// > unit file ...
/// > Only process and generate maintscripts for the installed unit files with the
/// > (base)name unit file.
/// >
/// > Note: dh_installsystemd will still install unit files from debian/ but it will not
/// > generate any maintscripts for them unless they are explicitly listed in unit file ...
#[derive(Default, Debug)]
pub struct Options {
    pub no_enable: bool,
    pub no_start: bool,
    pub restart_after_upgrade: bool,
    pub no_stop_on_upgrade: bool,
}

/// Find installable systemd unit files for the specified debian package (and
/// optional systemd unit name) in the given directory and return an install
/// recipe for each file detailing the path at which the file should be
/// installed and the mode (chmod) that the file should be given.
///
/// See:
///   <https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n264>
///   <https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n198>
///   <https://git.launchpad.net/ubuntu/+source/debhelper/tree/lib/Debian/Debhelper/Dh_Lib.pm?h=applied/12.10ubuntu1#n957>
pub fn find_units(dir: &Path, main_package: &str, unit_name: Option<&str>) -> PackageUnitFiles {
    let mut installables = HashMap::new();

    for (package_suffix, unit_type, install_dir) in SYSTEMD_UNIT_FILE_INSTALL_MAPPINGS.iter() {
        let package = &format!("{main_package}{package_suffix}");
        if let Some(src_path) = pkgfile(dir, main_package, package, unit_type, unit_name) {
            // .tmpfile files should be installed in a different directory and
            // with a different extension. See:
            //   https://www.freedesktop.org/software/systemd/man/tmpfiles.d.html
            let actual_suffix = match &unit_type[..] {
                "tmpfile" => "conf",
                _ => unit_type,
            };

            // Determine the file name that the unit file should be installed as
            // which depends on whether or not a unit name was provided.
            let install_filename = match unit_name {
                Some(name) => format!("{name}{package_suffix}.{actual_suffix}"),
                None => format!("{package}.{actual_suffix}"),
            };

            // Construct the full install path for this unit file.
            let install_path = Path::new(install_dir).join(install_filename);

            // Save the combination of source path, target path and target file
            // mode for this unit file.
            // eprintln!("[INFO] Identified installable at {:?}", src_path);
            installables.insert(
                src_path,
                InstallRecipe {
                    path: install_path,
                    mode: 0o644,
                },
            );
        }
    }

    installables
}

/// Determine if the given string is a systemd unit file comment line.
///
/// See:
///   <https://www.freedesktop.org/software/systemd/man/systemd.syntax.html#Introduction>
fn is_comment(s: &str) -> bool {
    matches!(s.chars().next(), Some('#') | Some(';'))
}

/// Strip off any first layer of outer quotes according to systemd quoting
/// rules.
///
/// See:
///   <https://www.freedesktop.org/software/systemd/man/systemd.service.html#Command%20lines>
fn unquote(s: &str) -> &str {
    if s.len() > 1 &&
       ((s.starts_with('"') && s.ends_with('"')) ||
       (s.starts_with('\'') && s.ends_with('\''))) {
        &s[1..s.len()-1]
    } else {
        s
    }
}

/// This function implements the primary logic of the Debian dh_installsystemd
/// Perl script, which is to say it identifies systemd units being installed,
/// inspects them and decides, based on the unit file and the configuration
/// options provided, which DebHelper autoscripts to use to correctly install
/// those units.
///
/// # Cargo Deb specific behaviour
///
/// Any `Asset`, whether identified by `find_units()` or added by the user
/// manually in Cargo.toml, that will be installed into `LIB_SYSTEMD_SYSTEM_DIR`
/// will be analysed.
///
/// Unlike `dh_installsystemd` results are returned as a `ScriptFragments` value
/// rather than being written to temporary files on disk.
///
/// # Usage
///
/// Pass the `ScriptFragments` result to `apply()`.
///
/// See:
///   <https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n288>
pub fn generate(package: &str, assets: &[Asset], options: &Options, listener: &dyn Listener) -> CDResult<ScriptFragments> {
    let mut scripts = ScriptFragments::new();

    // add postinst code blocks to handle tmpfiles
    // see: https://salsa.debian.org/debian/debhelper/-/blob/master/dh_installsystemd#L305
    let tmp_file_names = assets
        .iter()
        .filter(|a| a.c.target_path.starts_with(USR_LIB_TMPFILES_D_DIR))
        .map(|v| fname_from_path(v.source.path().unwrap()))
        .collect::<Vec<String>>()
        .join(" ");

    if !tmp_file_names.is_empty() {
        autoscript(&mut scripts, package, "postinst", "postinst-init-tmpfiles",
            &map!{ "TMPFILES" => tmp_file_names }, false, listener)?;
    }

    // add postinst, prerm, and postrm code blocks to handle activation,
    // deactivation, start and stopping of services when the package is
    // installed, upgraded or removed.
    // see: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n312

    // skip template service files. Enabling, disabling, starting or stopping
    // those services without specifying the instance is not useful.
    let mut installed_non_template_units: BTreeSet<String> = BTreeSet::new();
    installed_non_template_units.extend(
        assets
            .iter()
            .filter(|a| a.c.target_path.parent() == Some(LIB_SYSTEMD_SYSTEM_DIR.as_ref()))
            .map(|a| fname_from_path(a.c.target_path.as_path()))
            .filter(|fname| !fname.contains('@')),
    );

    // BTreeSets values iterate in sorted order irrespective of the order they
    // were inserted.
    // see: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n385
    let mut aliases = BTreeSet::new();
    let mut enable_units = BTreeSet::new();
    let mut start_units = BTreeSet::new();
    let mut seen = BTreeSet::new();

    // note: we do not support handling of services with a sysv-equivalent
    // see: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n373
    let mut units = installed_non_template_units;

    // for all installed non-template units and any units they refer to via
    // the 'Also=' key in their unit file, determine what if anything we need to
    // arrange to be done for them in the maintainer scripts.
    while !units.is_empty() {
        // gather unit names mentioned in 'Also=' kv pairs in the unit files
        let mut also_units = BTreeSet::<String>::new();

        // for each unit that we have not yet processed
        for unit in units.iter() {
            listener.info(format!("Determining augmentations needed for systemd unit {unit}"));

            // the unit has to be started
            start_units.insert(unit.clone());

            // get the unit file contents
            let needle = Path::new(LIB_SYSTEMD_SYSTEM_DIR).join(unit);
            let data = assets.iter().find(move |&item| item.c.target_path == needle).unwrap().source.data()?;
            let reader = data.into_owned();

            // for every line in the file look for specific keys that we are
            // interested in:
            // From: https://www.freedesktop.org/software/systemd/man/systemd.syntax.html
            //   "Each file is a plain text file divided into sections, with
            //    configuration entries in the style key=value. Whitespace
            //    immediately before or after the "=" is ignored. Empty lines
            //    and lines starting with "#" or ";" are ignored which may be
            //    used for commenting."
            //   "Various settings are allowed to be specified more than
            //    once"
            // Key names _seem_ to be case sensitive. It's not explicitly
            // stated in systemd.syntax.html above but this bug report seems
            // to confirm it:
            //   https://bugzilla.redhat.com/show_bug.cgi?id=846283
            // We also strip the value of any surrounding quotes because
            // that's what the actual dh_installsystemd code does:
            //   https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n210
            for line in reader.lines().map(|line| line.unwrap()).filter(|s| !is_comment(s)) {
                let possible_kv_pair = line.splitn(2, '=').map(|s| s.trim()).next_tuple();
                if let Some((key, value)) = possible_kv_pair {
                    let other_unit = unquote(value).to_string();
                    match key {
                        "Also" => {
                            // The seen lookup prevents us from looping forever over
                            // unit files that refer to each other. An actual
                            // real-world example of such a loop is systemd's
                            // systemd-readahead-drop.service, which contains
                            // Also=systemd-readahead-collect.service, and that file
                            // in turn contains Also=systemd-readahead-drop.service,
                            // thus forming an endless loop.
                            // see: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n340
                            if seen.insert(other_unit.clone()) {
                                also_units.insert(other_unit);
                            }
                        },
                        "Alias" => {
                            aliases.insert(other_unit);
                        },
                        _ => (),
                    };
                } else if line.starts_with("[Install]") {
                    enable_units.insert(unit.clone());
                }
            }
        }
        units = also_units;
    }

    // update the maintainer scripts to enable units unless forbidden by the
    // options passed to us.
    // see: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n390
    if !enable_units.is_empty() {
        let snippet = match options.no_enable {
            true => "postinst-systemd-dont-enable",
            false => "postinst-systemd-enable",
        };
        for unit in &enable_units {
            autoscript(&mut scripts, package, "postinst", snippet,
                &map!{ "UNITFILE" => unit.clone() }, true, listener)?;
        }
        autoscript(&mut scripts, package, "postrm", "postrm-systemd",
            &map!{ "UNITFILES" => enable_units.join(" ") }, false, listener)?;
    }

    // update the maintainer scripts to start units, where the exact action to
    // be taken is influenced by the options passed to us.
    // see: https://git.launchpad.net/ubuntu/+source/debhelper/tree/dh_installsystemd?h=applied/12.10ubuntu1#n398
    if !start_units.is_empty() {
        let mut replace = map! { "UNITFILES" => start_units.join(" ") };

        if options.restart_after_upgrade {
            let snippet;
            match options.no_start {
                true => {
                    snippet = "postinst-systemd-restartnostart";
                    replace.insert("RESTART_ACTION", "try-restart".into());
                },
                false => {
                    snippet = "postinst-systemd-restart";
                    replace.insert("RESTART_ACTION", "restart".into());
                }
            };
            autoscript(&mut scripts, package, "postinst", snippet, &replace, true, listener)?;
        } else if !options.no_start {
            // (stop|start) service (before|after) upgrade
            autoscript(&mut scripts, package, "postinst", "postinst-systemd-start", &replace, true, listener)?;
        }

        if options.no_stop_on_upgrade || options.restart_after_upgrade {
            // stop service only on remove
            autoscript(&mut scripts, package, "prerm", "prerm-systemd-restart", &replace, true, listener)?;
        } else if !options.no_start {
            // always stop service
            autoscript(&mut scripts, package, "prerm", "prerm-systemd", &replace, true, listener)?;
        }

        // Run this with "default" order so it is always after other service
        // related autosnippets.
		autoscript(&mut scripts, package, "postrm", "postrm-systemd-reload-only", &replace, false, listener)?;
    }

    Ok(scripts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Asset, AssetSource};
    use crate::util::tests::add_test_fs_paths;
    use crate::util::tests::get_read_count;
    use crate::util::tests::set_test_fs_path_content;
    use rstest::*;

    #[test]
    fn is_comment_detects_comments() {
        assert!(is_comment("#"));
        assert!(is_comment("#  "));
        assert!(is_comment("# some comment"));
        assert!(is_comment(";"));
        assert!(is_comment(";  "));
        assert!(is_comment("; some comment"));
    }

    #[test]
    fn is_comment_detects_non_comments() {
        assert!(!is_comment(" #"));
        assert!(!is_comment(" #  "));
        assert!(!is_comment(" # some comment"));
        assert!(!is_comment(" ;"));
        assert!(!is_comment(" ;  "));
        assert!(!is_comment(" ; some comment"));
    }

    #[test]
    fn unquote_unquotes_matching_single_quotes() {
        assert_eq!("", unquote("''"));
        assert_eq!("a", unquote("'a'"));
        assert_eq!("ab", unquote("'ab'"));
    }

    #[test]
    fn unquote_unquotes_matching_double_quotes() {
        assert_eq!("", unquote(r#""""#));
        assert_eq!("a", unquote(r#""a""#));
        assert_eq!("ab", unquote(r#""ab""#));
    }

    #[test]
    fn unquote_ignores_embedded_quotes() {
        assert_eq!("a'b", unquote("'a'b'"));
        assert_eq!(r#"a"b"#, unquote(r#"'a"b'"#));
        assert_eq!(r#"a"b"#, unquote(r#""a"b""#));
        assert_eq!(r#"a'b"#, unquote(r#""a'b""#));
    }

    #[test]
    fn unquote_ignores_partial_quotes() {
        assert_eq!("'", unquote("'"));
        assert_eq!("'ab", unquote("'ab"));
        assert_eq!("ab'", unquote("ab'"));
        assert_eq!("'ab'ab", unquote("'ab'ab"));
        assert_eq!("ab'ab'", unquote("ab'ab'"));
        assert_eq!(r#"""#, unquote(r#"""#));
        assert_eq!(r#""ab"#, unquote(r#""ab"#));
        assert_eq!(r#"ab""#, unquote(r#"ab""#));
        assert_eq!(r#""ab"ab"#, unquote(r#""ab"ab"#));
        assert_eq!(r#"ab"ab""#, unquote(r#"ab"ab""#));
    }

    #[test]
    fn unquote_ignores_mismatched_quotes() {
        assert_eq!(r#""'"#, unquote(r#""'"#));
        assert_eq!(r#"'""#, unquote(r#"'""#));
        assert_eq!(r#""a'"#, unquote(r#""a'"#));
        assert_eq!(r#"'a""#, unquote(r#"'a""#));
        assert_eq!(r#""ab'"#, unquote(r#""ab'"#));
        assert_eq!(r#"'ab""#, unquote(r#"'ab""#));
    }

    #[test]
    fn find_units_in_empty_dir_finds_nothing() {
        let pkg_unit_files = find_units(Path::new(""), "mypkg", None);
        assert!(pkg_unit_files.is_empty());
    }

    fn assert_eq_found_unit(pkg_unit_files: &PackageUnitFiles, expected_install_path: &str, source_path: &str) {
        let expected = InstallRecipe {
            path: PathBuf::from(expected_install_path),
            mode: 0o644,
        };
        let actual = pkg_unit_files.get(&PathBuf::from(source_path)).unwrap();
        assert_eq!(&expected, actual);
    }

    #[test]
    fn find_units_for_package() {
        // one of each valid pattern (without a specific unit) and one
        // additional valid pattern with a unit (which should not be matched
        // as we don't specify a specific unit name to match)
        let _g = add_test_fs_paths(&[
            "debian/mypkg.mount",
            "debian/mypkg@.path",
            "debian/service", // demonstrates the main package fallback
            "debian/mypkg@.socket",
            "debian/mypkg.target",
            "debian/mypkg@.timer",
            "debian/mypkg.tmpfile",
            "debian/mypkg.myunit.service", // demonstrates lack of unit name
        ]);
        let pkg_unit_files = find_units(Path::new("debian"), "mypkg", None);
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/mypkg.mount",   "debian/mypkg.mount");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/mypkg@.path",   "debian/mypkg@.path");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/mypkg.service", "debian/service");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/mypkg@.socket", "debian/mypkg@.socket");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/mypkg.target",  "debian/mypkg.target");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/mypkg@.timer",  "debian/mypkg@.timer");
        assert_eq_found_unit(&pkg_unit_files, "usr/lib/tmpfiles.d/mypkg.conf",    "debian/mypkg.tmpfile");
        assert_eq!(7, pkg_unit_files.len());
    }

    #[test]
    fn find_named_units_for_package() {
        // one of each valid pattern (with a specific unit) and one additional
        // valid pattern without a unit (which should not be matched if there is
        // match with the correctly named unit).
        let _g = add_test_fs_paths(&[
            "debian/mypkg.myunit.mount",
            "debian/mypkg@.myunit.path",
            "debian/service", // main package match should be ignored
            "debian/mypkg@.myunit.socket",
            "debian/target", // no unit or package but should be matched as fallback
            "debian/mypkg@.myunit.timer",
            "debian/mypkg.tmpfile", // no unit but should be matched as fallback
            "debian/mypkg.myunit.service", // should be matched over main package match above
        ]);

        // add some paths that should not be matched
        let _g = add_test_fs_paths(&[
            "debian/nested/dir/mykpg.myunit.mount",
            "debian/README.md",
            "mypkg.myunit.mount",
            "mypkg.mount",
            "mount",
            "postinit",
            "mypkg.postinit",
            "mypkg.myunit.postinit",
        ]);

        let pkg_unit_files = find_units(Path::new("debian"), "mypkg", Some("myunit"));
        // note the "myunit" target names, even when the match was less specific
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/myunit.mount",   "debian/mypkg.myunit.mount");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/myunit@.path",   "debian/mypkg@.myunit.path");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/myunit.service", "debian/mypkg.myunit.service");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/myunit@.socket", "debian/mypkg@.myunit.socket");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/myunit.target",  "debian/target");
        assert_eq_found_unit(&pkg_unit_files, "lib/systemd/system/myunit@.timer",  "debian/mypkg@.myunit.timer");

        // note the changed file extension
        assert_eq_found_unit(&pkg_unit_files, "usr/lib/tmpfiles.d/myunit.conf",    "debian/mypkg.tmpfile");

        assert_eq!(7, pkg_unit_files.len());
    }

    #[test]
    fn generate_with_empty_inputs_does_nothing() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());

        let fragments = generate("", &[], &Options::default(), &mock_listener).unwrap();

        assert!(fragments.is_empty());
    }

    #[test]
    fn generate_with_arbitrary_asset_does_nothing() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());

        let assets = vec![Asset::new(
            AssetSource::Path(PathBuf::new()),
            PathBuf::new(),
            0o0,
            crate::manifest::IsBuilt::No,
        )];

        let fragments = generate("mypkg", &assets, &Options::default(), &mock_listener).unwrap();
        assert!(fragments.is_empty());
    }

    #[test]
    #[should_panic(expected = "unwrap")]
    fn generate_with_invalid_tmp_file_asset_panics() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());

        let assets = vec![Asset::new(
            AssetSource::Path(PathBuf::new()), // path source with empty source path makes no sense
            Path::new("usr/lib/tmpfiles.d/blah").to_path_buf(),
            0o0,
            crate::manifest::IsBuilt::No,
        )];

        let fragments = generate("mypkg", &assets, &Options::default(), &mock_listener).unwrap();
        assert!(fragments.is_empty());
    }

    #[test]
    #[should_panic(expected = "unwrap")]
    fn generate_with_data_tmp_file_asset_panics() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());

        let assets = vec![Asset::new(
            AssetSource::Data(vec![]), // only assets of type Path are currently supported
            Path::new("usr/lib/tmpfiles.d/blah").to_path_buf(),
            0o0,
            crate::manifest::IsBuilt::No,
        )];

        let fragments = generate("mypkg", &assets, &Options::default(), &mock_listener).unwrap();
        assert!(fragments.is_empty());
    }

    #[test]
    fn generate_with_empty_tmp_file_asset() {
        const TMP_FILE_NAME: &str = "my_tmp_file";
        let tmp_file_path = PathBuf::from(format!("debian/{TMP_FILE_NAME}"));

        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(1).return_const(());

        let assets = vec![Asset::new(
            AssetSource::Path(tmp_file_path),
            Path::new("usr/lib/tmpfiles.d/blah").to_path_buf(),
            0o0,
            crate::manifest::IsBuilt::No,
        )];

        let fragments = generate("mypkg", &assets, &Options::default(), &mock_listener).unwrap();
        assert_eq!(1, fragments.len());

        let (fragment_name, fragment_bytes) = fragments.into_iter().next().unwrap();

        // should create an augmentation for the postinst script
        assert_eq!("mypkg.postinst.debhelper", fragment_name);

        // Verify the created script contents. It should have two lines
        // more than the autoscript fragment it was based on, like so:
        //   # Automatically added by ...
        //   <autoscript fragment lines with placeholders replaced>
        //   # End automatically added section
        let autoscript_text = get_embedded_autoscript("postinst-init-tmpfiles");
        let autoscript_line_count = autoscript_text.lines().count();
        let created_text = String::from_utf8(fragment_bytes).unwrap();
        let created_line_count = created_text.lines().count();
        assert_eq!(autoscript_line_count + 2, created_line_count);

        // Verify the content of the added comment lines
        let mut lines = created_text.lines();
        assert!(lines.next().unwrap().starts_with("# Automatically added by"));
        assert_eq!(lines.nth_back(0).unwrap(), "# End automatically added section");

        // Check that the autoscript fragment lines were properly copied
        // into the created script complete with expected substitutions
        let expected_autoscript_text = autoscript_text.replace("#TMPFILES#", TMP_FILE_NAME);
        let expected_autoscript_text = expected_autoscript_text.trim_end();
        let start1 = 1;
        let end1 = start1 + autoscript_line_count;
        let created_autoscript_text = created_text.lines().collect::<Vec<&str>>()[start1..end1].join("\n");
        assert_ne!(expected_autoscript_text, autoscript_text);
        assert_eq!(expected_autoscript_text, created_autoscript_text);
    }

    #[test]
    fn generate_filters_out_template_units() {
        // "A template unit must have a single "@" at the end of the name
        // (right before the type suffix)" - from:
        //   https://www.freedesktop.org/software/systemd/man/systemd.unit.html
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());

        let assets = vec![Asset::new(
            AssetSource::Path(PathBuf::from("debian/my_unit@.service")),
            Path::new("lib/systemd/system/").to_path_buf(),
            0o0,
            crate::manifest::IsBuilt::No,
        )];

        let fragments = generate("mypkg", &assets, &Options::default(), &mock_listener).unwrap();
        assert_eq!(0, fragments.len());
    }

    #[test]
    fn generate_filters_out_subdir() {
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());

        let assets = vec![Asset::new(
            AssetSource::Path(PathBuf::from("debian/10-extra-hardening.conf")),
            Path::new("lib/systemd/system/foobar.service.d/").to_path_buf(),
            0o0,
            crate::manifest::IsBuilt::No,
        )];

        let fragments = generate("mypkg", &assets, &Options::default(), &mock_listener).unwrap();
        assert_eq!(0, fragments.len());
    }

    #[test]
    fn generate_acts_only_on_unit_files_with_the_expected_install_path() {
        // Note: find_units() will set the target path correctly.
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().times(0).return_const(());

        let assets = vec![Asset::new(
            AssetSource::Path(PathBuf::from("debian/my_unit.service")),
            Path::new("some/other/path/").to_path_buf(),
            0o0,
            crate::manifest::IsBuilt::No,
        )];

        let fragments = generate("mypkg", &assets, &Options::default(), &mock_listener).unwrap();
        assert_eq!(0, fragments.len());
    }

    #[rstest(ip, inst, ne, rau, ns, nsou,
      case("ult", false, false, false, false, false),

      case("lss", false, false, false, false, false),
      case("lss", false, false, false, false, true),
      case("lss", false, false, false, true,  false),
      case("lss", false, false, false, true,  true),
      case("lss", false, false, true,  false, false),
      case("lss", false, false, true,  false,  true),
      case("lss", false, false, true,  true,  false),
      case("lss", false, false, true,  true,  true),
      case("lss", false, true,  false, false, false),
      case("lss", false, true,  false, false, true),
      case("lss", false, true,  false, true,  false),
      case("lss", false, true,  false, true,  true),
      case("lss", false, true,  true,  false, false),
      case("lss", false, true,  true,  false,  true),
      case("lss", false, true,  true,  true,  false),
      case("lss", false, true,  true,  true,  true),
      case("lss", true,  false, false, false, false),
      case("lss", true,  false, false, false, true),
      case("lss", true,  false, false, true,  false),
      case("lss", true,  false, false, true,  true),
      case("lss", true,  false, true,  false, false),
      case("lss", true,  false, true,  false,  true),
      case("lss", true,  false, true,  true,  false),
      case("lss", true,  false, true,  true,  true),
      case("lss", true,  true,  false, false, false),
      case("lss", true,  true,  false, false, true),
      case("lss", true,  true,  false, true,  false),
      case("lss", true,  true,  false, true,  true),
      case("lss", true,  true,  true,  false, false),
      case("lss", true,  true,  true,  false,  true),
      case("lss", true,  true,  true,  true,  false),
      case("lss", true,  true,  true,  true,  true),
    )]
    #[test]
    fn generate_creates_expected_autoscript_fragments(
        ip: &str,
        inst: bool,
        ne: bool,
        rau: bool,
        ns: bool,
        nsou: bool,
    ) {
        let unit_file_path = "debian/mypkg.service";

        let install_base_path = match ip {
            "ult" => "usr/lib/tmpfiles.d",
            "lss" => "lib/systemd/system",
            x => panic!("Unsupported install path value '{x}'"),
        };

        // setup input for generate()
        let assets = vec![Asset::new(
            AssetSource::Path(PathBuf::from(unit_file_path)),
            format!("{install_base_path}/mypkg.service").into(),
            0o0,
            crate::manifest::IsBuilt::No,
        )];

        let options = Options {
            no_enable: ne,
            no_start: ns,
            restart_after_upgrade: rau,
            no_stop_on_upgrade: nsou,
        };

        // setup mocks
        let mut mock_listener = crate::listener::MockListener::new();
        mock_listener.expect_info().return_const(());

        // start_units: yes
        // enable_units: no, no [Install] section in the unit file

        let mut unit_file_content = "[Unit]
Description=A test unit

[Service]
Type=simple
".to_owned();

        if inst {
            unit_file_content.push_str("[Install]
WantedBy=multi-user.target");
        }

        set_test_fs_path_content(unit_file_path, unit_file_content);

        // Add all Autoscript paths to the in-memory test file system so that
        // we can track whether they are read or not.
        let _g = add_test_fs_paths(&[
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
        ]);

        // generate!
        let fragments = generate("mypkg", &assets, &options, &mock_listener).unwrap();

        // verify, though don't verify creation of autoscript fragments as that
        // is verified in tests of the lower level functionality, instead verify
        // only that the generate() logic creates the expected named fragments
        // and while doing so read the expected autoscript files the expected
        // number of times.

        // Perl dh_installsystemd logic selects autoscript fragments based on
        // the following conditions. If multiple columns have entries then all
        // must be true. If a column has no value it is always true for all
        // units.
        //
        // key:
        //   - ip    - install path
        //     - lss - lib/systemd/system/
        //     - ult - usr/lib/tmpfiles.d/
        //   - [I]   - has an [Install] section in the unit file
        //   - ne    - the value of the boolean no_enable option
        //   - rau   - the value of the boolean restart_after_upgrade option
        //   - ns    - the value of the boolean no_start option
        //   - nsou  - the value of the boolean no_stop_on_upgrade option
        //   - /     - true/present (/* denotes one true is enough)
        //   - x     - false/missing
        //   - tr    - try_restart (value of #RESTART_ACTION# placeholder)
        //   - r     - restart (value of #RESTART_ACTION# placeholder)
        //
        // -----------------------------------------------------------------------
        // autoscript fragment             | ip  | [I] | ne | rau    | ns | nsou |
        // -----------------------------------------------------------------------
        // postinst-init-tmpfiles          | ult |     |    |        |    |      |
        // postinst-systemd-dont-enable    | lss | /   | /  |        |    |      |
        // postinst-systemd-enable         | lss | /   | x  |        |    |      |
        // postinst-systemd-restart        | lss |     |    | / (tr) | x  |      |
        // postinst-systemd-restartnostart | lss |     |    | / (r)  | /  |      |
        // postinst-systemd-start          | lss |     |    | x      | x  |      |
        // postrm-systemd                  | lss | /   |    |        |    |      |
        // postrm-systemd-reload-only      | lss |     |    |        |    |      |
        // prerm-systemd                   | lss |     |    | x      | x  | x    |
        // prerm-systemd-restart           | lss |     |    | /*     |    | /*   |
        // -----------------------------------------------------------------------

        let mut autoscript_fragments_to_check_for = std::collections::HashSet::new();

        match ip {
            "ult" => {
                assert_eq!(1, get_read_count("postinst-init-tmpfiles"));
                autoscript_fragments_to_check_for.insert("postinst.debhelper");
            },
            "lss" => {
                assert_eq!(1, get_read_count(unit_file_path));
                if inst {
                    match options.no_enable {
                        true => assert_eq!(1, get_read_count("postinst-systemd-dont-enable")),
                        false => assert_eq!(1, get_read_count("postinst-systemd-enable")),
                    };
                    assert_eq!(1, get_read_count("postrm-systemd"));
                    autoscript_fragments_to_check_for.insert("postinst.service");
                    autoscript_fragments_to_check_for.insert("postrm.debhelper");
                }
                match options.restart_after_upgrade {
                    true => {
                        match options.no_start {
                            true => assert_eq!(1, get_read_count("postinst-systemd-restartnostart")),
                            false => assert_eq!(1, get_read_count("postinst-systemd-restart")),
                        };
                        autoscript_fragments_to_check_for.insert("postinst.service");
                    },
                    false => if !options.no_start {
                        assert_eq!(1, get_read_count("postinst-systemd-start"));
                        autoscript_fragments_to_check_for.insert("postinst.service");
                    },
                }
                if options.restart_after_upgrade || options.no_stop_on_upgrade {
                    assert_eq!(1, get_read_count("prerm-systemd-restart"));
                    autoscript_fragments_to_check_for.insert("prerm.service");
                } else if !options.no_start {
                    assert_eq!(1, get_read_count("prerm-systemd"));
                    autoscript_fragments_to_check_for.insert("prerm.service");
                }
                assert_eq!(1, get_read_count("postrm-systemd-reload-only"));
                autoscript_fragments_to_check_for.insert("postrm.debhelper");
            },
            _ => unreachable!(),
        }

        for autoscript in autoscript_fragments_to_check_for.iter() {
            let key = format!("mypkg.{}", autoscript);
            assert!(fragments.contains_key(&key), "{}", key);
        }
    }
}
