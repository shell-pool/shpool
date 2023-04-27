use std::env::consts::DLL_PREFIX;
use std::env::consts::DLL_SUFFIX;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

#[test]
fn build_workspaces() {
    let (cdir, ddir) = extract_built_package_from_manifest("tests/test-workspace/test-ws1/Cargo.toml", &["--no-strip", "--fast"]);
    assert!(ddir.path().join("usr/local/bin/renamed2").exists());
    assert!(ddir.path().join("usr/local/bin/decoy").exists());

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Version: 1.0.0-ws\n"));
    assert!(control.contains("Package: test1-crate-name\n"));
    assert!(control.contains("Maintainer: ws\n"));

    let (_, ddir) = extract_built_package_from_manifest("tests/test-workspace/test-ws2/Cargo.toml", &["--no-strip"]);
    assert!(ddir.path().join("usr/bin/renamed2").exists());
    assert!(ddir.path().join(format!("usr/lib/{DLL_PREFIX}test2lib{DLL_SUFFIX}")).exists());
}

#[track_caller]
fn extract_built_package_from_manifest(manifest_path: &str, args: &[&str]) -> (TempDir, TempDir) {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let cmd_path = root.join(env!("CARGO_BIN_EXE_cargo-deb"));
    assert!(cmd_path.exists());
    let output = Command::new(cmd_path)
        .arg(format!("--manifest-path={}", root.join(manifest_path).display()))
        .args(args)
        .output().unwrap();
    if !output.status.success() {
        panic!("Cmd failed: {}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }

    // prints deb path on the last line
    let last_line = output.stdout[..output.stdout.len() - 1].split(|&c| c == b'\n').last().unwrap();
    let deb_path = Path::new(::std::str::from_utf8(last_line).unwrap());
    assert!(deb_path.exists());

    let ardir = tempfile::tempdir().expect("testdir");
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    assert_eq!("2.0\n", fs::read_to_string(ardir.path().join("debian-binary")).unwrap());

    let ext = if cfg!(feature = "lzma") { "xz" } else { "gz" };
    assert!(ardir.path().join(format!("data.tar.{ext}")).exists());
    assert!(ardir.path().join(format!("control.tar.{ext}")).exists());

    let cdir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xf")
        .current_dir(cdir.path())
        .arg(ardir.path().join(format!("control.tar.{ext}")))
        .status().unwrap().success());

    let ddir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(ddir.path())
        .arg(ardir.path().join(format!("data.tar.{ext}")))
        .status().unwrap().success());

    (cdir, ddir)
}

#[test]
#[cfg(all(feature = "lzma", target_os = "linux"))]
fn run_cargo_deb_command_on_example_dir() {

    let (cdir, ddir) = extract_built_package_from_manifest("example/Cargo.toml", &[]);

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example\n"));
    assert!(control.contains("Version: 0.1.0\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let md5sums = fs::read_to_string(cdir.path().join("md5sums")).unwrap();
    assert!(md5sums.contains(" usr/bin/example\n"));
    assert!(md5sums.contains(" usr/share/doc/example/changelog.Debian.gz\n"));
    assert!(md5sums.contains("b1946ac92492d2347c6235b4d2611184  var/lib/example/1.txt\n"));
    assert!(md5sums.contains("591785b794601e212b260e25925636fd  var/lib/example/2.txt\n"));
    assert!(md5sums.contains("1537684900f6b12358c88a612adf1049  var/lib/example/3.txt\n"));
    assert!(md5sums.contains("6f65f1e8907ea8a25171915b3bba45af  usr/share/doc/example/copyright\n"));

    assert!(ddir.path().join("var/lib/example/1.txt").exists());
    assert!(ddir.path().join("var/lib/example/2.txt").exists());
    assert!(ddir.path().join("var/lib/example/3.txt").exists());
    assert!(ddir.path().join("usr/share/doc/example/copyright").exists());
    assert!(ddir.path().join("usr/share/doc/example/changelog.Debian.gz").exists());
    assert!(ddir.path().join("usr/bin/example").exists());
    // changelog.Debian.gz starts with the gzip magic
    assert_eq!(
        &[0x1F, 0x8B],
        &fs::read(ddir.path().join("usr/share/doc/example/changelog.Debian.gz")).unwrap()[..2]
    );
}

#[test]
#[cfg(all(feature = "lzma"))]
fn run_cargo_deb_command_on_example_dir_with_variant() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cmd_path = root.join(env!("CARGO_BIN_EXE_cargo-deb"));
    assert!(cmd_path.exists());
    let cargo_dir = tempfile::tempdir().unwrap();
    let deb_path = cargo_dir.path().join("test.deb");
    let output = Command::new(cmd_path)
        .env("CARGO_TARGET_DIR", cargo_dir.path()) // otherwise tests overwrite each other
        .arg("--variant=debug")
        .arg("--no-strip")
        .arg(format!("--output={}", deb_path.display()))
        .arg(format!(
            "--manifest-path={}",
            root.join("example/Cargo.toml").display()
        ))
        .output()
        .unwrap();
    if !output.status.success() {
        panic!("Cmd failed: {}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }

    // prints deb path on the last line
    let last_line = output.stdout[..output.stdout.len() - 1].split(|&c| c == b'\n').last().unwrap();
    let printed_deb_path = Path::new(::std::str::from_utf8(last_line).unwrap());
    assert_eq!(printed_deb_path, deb_path);
    assert!(deb_path.exists());

    let ardir = tempfile::tempdir().unwrap();
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    assert_eq!("2.0\n", fs::read_to_string(ardir.path().join("debian-binary")).unwrap());
    let ext = if cfg!(feature = "lzma") { "xz" } else { "gz" };
    assert!(ardir.path().join(format!("data.tar.{ext}")).exists());
    assert!(ardir.path().join(format!("control.tar.{ext}")).exists());

    let cdir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(cdir.path())
        .arg(ardir.path().join(format!("control.tar.{ext}")))
        .status().unwrap().success());

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example-debug\n"), "Control is: {:?}", control);
    assert!(control.contains("Version: 0.1.0\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let md5sums = fs::read_to_string(cdir.path().join("md5sums")).unwrap();
    assert!(md5sums.contains(" usr/bin/example\n"));
    assert!(md5sums.contains(" usr/share/doc/example-debug/changelog.Debian.gz\n"));
    assert!(md5sums.contains("b1946ac92492d2347c6235b4d2611184  var/lib/example/1.txt\n"));
    assert!(md5sums.contains("591785b794601e212b260e25925636fd  var/lib/example/2.txt\n"));
    assert!(md5sums.contains("835a3c46f2330925774ebf780aa74241  var/lib/example/4.txt\n"));
    assert!(md5sums.contains("2455967cef930e647146a8c762199ed3  usr/share/doc/example-debug/copyright\n"));

    let ddir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(ddir.path())
        .arg(ardir.path().join(format!("data.tar.{ext}")))
        .status().unwrap().success());

    assert!(ddir.path().join("var/lib/example/1.txt").exists());
    assert!(ddir.path().join("var/lib/example/2.txt").exists());
    assert!(ddir.path().join("var/lib/example/4.txt").exists());
    assert!(ddir.path().join("usr/share/doc/example-debug/copyright").exists());
    assert!(ddir.path().join("usr/share/doc/example-debug/changelog.Debian.gz").exists());
    assert!(ddir.path().join("usr/bin/example").exists());
}

#[test]
#[cfg(all(feature = "lzma", target_os = "linux"))]
fn run_cargo_deb_command_on_example_dir_with_version() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let cmd_path = root.join(env!("CARGO_BIN_EXE_cargo-deb"));
    assert!(cmd_path.exists());
    let cargo_dir = tempfile::tempdir().unwrap();
    let deb_path = cargo_dir.path().join("test.deb");
    let output = Command::new(cmd_path)
        .env("CARGO_TARGET_DIR", cargo_dir.path()) // otherwise tests overwrite each other
        .arg(format!("--output={}", deb_path.display()))
        .arg(format!(
            "--manifest-path={}",
            root.join("example/Cargo.toml").display()
        ))
        .arg(format!("--deb-version=my-custom-version"))
        .output().unwrap();
    assert!(output.status.success());

    // prints deb path on the last line
    let last_line = output.stdout[..output.stdout.len() - 1].split(|&c| c == b'\n').last().unwrap();
    let deb_path = Path::new(::std::str::from_utf8(last_line).unwrap());
    assert!(deb_path.exists());

    let ardir = tempfile::tempdir().unwrap();
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    let ext = if cfg!(feature = "lzma") { "xz" } else { "gz" };
    assert_eq!("2.0\n", fs::read_to_string(ardir.path().join("debian-binary")).unwrap());
    assert!(ardir.path().join(format!("data.tar.{ext}")).exists());
    assert!(ardir.path().join(format!("control.tar.{ext}")).exists());

    let cdir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xf")
        .current_dir(cdir.path())
        .arg(ardir.path().join(format!("control.tar.{ext}")))
        .status().unwrap().success());

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example\n"));
    assert!(control.contains("Version: my-custom-version\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let md5sums = fs::read_to_string(cdir.path().join("md5sums")).unwrap();
    assert!(md5sums.contains(" usr/bin/example\n"));
    assert!(md5sums.contains(" usr/share/doc/example/changelog.Debian.gz\n"));
    assert!(md5sums.contains("b1946ac92492d2347c6235b4d2611184  var/lib/example/1.txt\n"));
    assert!(md5sums.contains("591785b794601e212b260e25925636fd  var/lib/example/2.txt\n"));
    assert!(md5sums.contains("1537684900f6b12358c88a612adf1049  var/lib/example/3.txt\n"));
    assert!(md5sums.contains("6f65f1e8907ea8a25171915b3bba45af  usr/share/doc/example/copyright\n"), "has:\n{}", md5sums);

    let ddir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(ddir.path())
        .arg(ardir.path().join(format!("data.tar.{ext}")))
        .status().unwrap().success());

    assert!(ddir.path().join("var/lib/example/1.txt").exists());
    assert!(ddir.path().join("var/lib/example/2.txt").exists());
    assert!(ddir.path().join("var/lib/example/3.txt").exists());
    assert!(ddir.path().join("usr/share/doc/example/copyright").exists());
    assert!(ddir.path().join("usr/share/doc/example/changelog.Debian.gz").exists());
    assert!(ddir.path().join("usr/bin/example").exists());
    // changelog.Debian.gz starts with the gzip magic
    assert_eq!(
        &[0x1F, 0x8B],
        &fs::read(ddir.path().join("usr/share/doc/example/changelog.Debian.gz")).unwrap()[..2]
    );
}
