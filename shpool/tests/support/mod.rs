// This module is used from multiple different test files, each of which
// gets compiled into its own binary. Not all the binaries use all the
// stuff here.
#![allow(dead_code)]

use std::{
    env, io,
    io::BufRead,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time,
};

use anyhow::{anyhow, Context};

pub mod attach;
pub mod daemon;
pub mod events;
pub mod line_matcher;
pub mod tmpdir;

pub fn testdata_file<P: AsRef<Path>>(file: P) -> PathBuf {
    let mut dir = cargo_dir();
    dir.pop();
    dir.pop();
    dir.join("shpool").join("tests").join("data").join(file)
}

lazy_static::lazy_static! {
    // cache the result and make sure we only ever compile once
    static ref SHPOOL_BIN_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);
}

pub fn wait_until<P>(mut pred: P) -> anyhow::Result<()>
where
    P: FnMut() -> anyhow::Result<bool>,
{
    let mut sleep_dur = time::Duration::from_millis(5);
    for _ in 0..12 {
        if pred()? {
            return Ok(());
        } else {
            std::thread::sleep(sleep_dur);
            sleep_dur *= 2;
        }
    }

    Err(anyhow!("pred never became true"))
}

pub fn shpool_bin() -> anyhow::Result<PathBuf> {
    let mut cached = SHPOOL_BIN_PATH.lock().unwrap();
    if let Some(path) = &*cached {
        return Ok(path.to_path_buf());
    }

    let mut project_dir = cargo_dir();
    project_dir.pop();
    project_dir.pop();

    let out = Command::new("cargo")
        .arg("build")
        .arg("--features=test_hooks")
        .arg("--message-format=json")
        .current_dir(project_dir)
        .output()
        .context("scraping cargo test binaries")?;

    if !out.status.success() {
        return Err(anyhow!("cargo invocation failed"));
    }

    let line_reader = io::BufReader::new(&out.stdout[..]);
    for line in line_reader.lines() {
        let line = line.context("reading line from stdout")?;
        let entry: serde_json::Value =
            serde_json::from_str(&line).context("parsing an output line from cargo")?;

        let src_path = entry.get("target").and_then(|v| v.get("src_path")).and_then(|v| v.as_str());
        let exe = entry.get("executable").and_then(|v| v.as_str());
        let kind = entry
            .get("target")
            .and_then(|v| v.get("kind"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str());
        let crate_type = entry
            .get("target")
            .and_then(|v| v.get("crate_types"))
            .and_then(|v| v.get(0))
            .and_then(|v| v.as_str());

        if let (Some(src_path), Some(exe), Some(kind), Some(crate_type)) =
            (src_path, exe, kind, crate_type)
        {
            if !src_path.ends_with("src/main.rs") {
                continue;
            }

            if kind != "bin" {
                continue;
            }

            if crate_type != "bin" {
                continue;
            }

            if let Some(exe_basename) = Path::new(&exe).file_name() {
                if exe_basename.to_os_string().into_string().unwrap() != "shpool" {
                    eprintln!("shpool bin 5");
                    continue;
                }
            } else {
                continue;
            }

            let path = PathBuf::from(exe);
            *cached = Some(path.clone());
            return Ok(path);
        }
    }

    Err(anyhow!("could not find shpool bin name"))
}

pub fn cargo_dir() -> PathBuf {
    env::var_os("CARGO_BIN_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            env::current_exe().ok().map(|mut path| {
                path.pop();
                if path.ends_with("deps") {
                    path.pop();
                }
                path
            })
        })
        .unwrap_or_else(|| panic!("CARGO_BIN_PATH wasn't set. Cannot continue running test"))
}
