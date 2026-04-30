use std::path::PathBuf;

/// The path of the current exe, without any funny business.
pub fn current() -> anyhow::Result<PathBuf> {
    let path = std::env::current_exe()?;

    // The linux kernel will append " (deleted)" to the path that
    // /proc/<pid>/exe links to when the binary file gets deleted.
    // This happens on package update when a new version is copied
    // into place, so we need to handle it. We'll just assume that
    // the new file that replaced us is a new shpool version.
    if cfg!(target_os = "linux") {
        if let Some(path_str) = path.to_str() {
            if let Some(stripped) = path_str.strip_suffix(" (deleted)") {
                return Ok(PathBuf::from(stripped));
            }
        }
    }

    Ok(path)
}
