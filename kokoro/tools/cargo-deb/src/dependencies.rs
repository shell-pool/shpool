use crate::debian_triple_from_rust_triple;
use crate::error::*;
use std::io::BufRead;
use std::path::Path;
use std::process::Command;

/// Resolves the dependencies based on the output of dpkg-shlibdeps on the binary.
pub fn resolve(path: &Path, target: &Option<String>) -> CDResult<Vec<String>> {
    let temp_folder = tempfile::tempdir()?;
    let debian_folder = temp_folder.path().join("debian");
    let control_file_path = debian_folder.join("control");
    std::fs::create_dir_all(&debian_folder)?;
    // dpkg-shlibdeps requires a (possibly empty) debian/control file to exist in its working
    // directory. The executable location doesn't matter.
    let _ = std::fs::File::create(&control_file_path);

    // Print result to stdout instead of a file.
    let mut args = vec!["-O"];
    let libpath_arg;
    // determine library search path from target
    if let Some(target) = target {
        libpath_arg = format!("-l/usr/{}/lib", debian_triple_from_rust_triple(target));
        args.push(&libpath_arg);
    }
    const DPKG_SHLIBDEPS_COMMAND: &str = "dpkg-shlibdeps";
    let output = Command::new(DPKG_SHLIBDEPS_COMMAND)
        .args(args)
        .arg(path)
        .current_dir(temp_folder.path())
        .output()
        .map_err(|e| CargoDebError::CommandFailed(e, DPKG_SHLIBDEPS_COMMAND))?;
    if !output.status.success() {
        return Err(CargoDebError::CommandError(
            DPKG_SHLIBDEPS_COMMAND,
            path.display().to_string(),
            output.stderr,
        ));
    }

    log::debug!("dpkg-shlibdeps for {}: {}", path.display(), String::from_utf8_lossy(&output.stdout));

    let deps = output.stdout.lines()
        .filter_map(|line| line.ok())
        .find(|line| line.starts_with("shlibs:Depends="))
        .ok_or(CargoDebError::Str("Failed to find dependency specification."))?
        .trim_start_matches("shlibs:Depends=")
        .split(',')
        .map(|dep| dep.trim().to_string())
        // libgcc guaranteed by LSB to always be present
        .filter(|dep| !dep.starts_with("libgcc-") && !dep.starts_with("libgcc1-"))
        .collect();

    Ok(deps)
}

#[test]
#[cfg(target_os = "linux")]
fn resolve_test() {
    let exe = std::env::current_exe().unwrap();
    let deps = resolve(&exe, &None).unwrap();
    assert!(deps.iter().any(|d| d.starts_with("libc")));
    assert!(!deps.iter().any(|d| d.starts_with("libgcc")));
}
