use std::io::Read;
use std::process::{Command, Stdio};
use std::time;

use anyhow::{Context, anyhow};
use tempdir::TempDir;

mod support;

// A basic smoke test
#[test]
fn start_test() -> anyhow::Result<()> {
    let tmp_dir = TempDir::new("shpool-test").context("creating tmp dir")?;

    let mut child = Command::new(support::shpool_bin())
        .stdout(Stdio::piped())
        .arg("daemon")
        .arg("--socket").arg(tmp_dir.path().join("shpool.socket"))
        .arg("--config-file").arg(support::testdata_file("empty.toml"))
        .spawn()
        .context("spawning daemon process")?;

    // The server should start up and run without incident for
    // half a second.
    std::thread::sleep(time::Duration::from_millis(500));

    child.kill().context("killing child")?;

    let mut stdout = child.stdout.take().context("missing stderr")?;
    let mut stdout_str = String::from("");
    stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;

    if stdout_str != "" {
        println!("{}", stdout_str);
        return Err(anyhow!("unexpected stderr output"));
    }

    Ok(())
}
