use std::process;

use anyhow::{anyhow, Context};

#[derive(Debug)]
pub struct Info {
    pub default_shell: String,
    pub home_dir: String,
}

pub fn info() -> anyhow::Result<Info> {
    let out = process::Command::new("/bin/sh")
        .arg("-c")
        .arg("cd ; echo \"$SHELL|$PWD\"")
        .output()
        .context("spawning subshell to determine default shell")?;
    if !out.status.success() {
        return Err(anyhow!("bad status checking for default shell: {}", out.status));
    }
    if out.stderr.len() != 0 {
        return Err(anyhow!("unexpected stderr when checking for default shell: {}",
                           String::from_utf8_lossy(&out.stderr)));
    }

    let parts = String::from_utf8(out.stdout.clone())
        .context("parsing default shell as utf8")?
        .trim().split("|").map(String::from).collect::<Vec<String>>();
    if parts.len() != 2 {
        return Err(anyhow!("could not parse output: '{}'", 
                           String::from_utf8_lossy(&out.stdout)));
    }
    Ok(Info {
        default_shell: parts[0].clone(),
        home_dir: parts[1].clone(),
    })
}
