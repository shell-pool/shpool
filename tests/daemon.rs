use std::io::Read;
use std::process::{Command, Stdio};
use std::time;

use anyhow::{Context, anyhow};

mod support;

#[test]
fn start() -> anyhow::Result<()> {
    let tmp_dir = tempfile::Builder::new().prefix("shpool-test").rand_bytes(20)
        .tempdir().context("creating tmp dir")?;

    let mut child = Command::new(support::shpool_bin())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--socket").arg(tmp_dir.path().join("shpool.socket"))
        .arg("daemon")
        .spawn()
        .context("spawning daemon process")?;

    // The server should start up and run without incident for
    // half a second.
    std::thread::sleep(time::Duration::from_millis(500));

    child.kill().context("killing child")?;

    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stdout_str = String::from("");
    stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;

    if stdout_str != "" {
        println!("{}", stdout_str);
        return Err(anyhow!("unexpected stdout output"));
    }

    let mut stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_str = String::from("");
    stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
    assert!(stderr_str.contains("STARTING DAEMON"));

    Ok(())
}

#[test]
fn config() -> anyhow::Result<()> {
    let tmp_dir = tempfile::Builder::new().prefix("shpool-test").rand_bytes(20)
        .tempdir().context("creating tmp dir")?;

    let mut child = Command::new(support::shpool_bin())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--socket").arg(tmp_dir.path().join("shpool.socket"))
        .arg("daemon")
        .arg("--config-file").arg(support::testdata_file("empty.toml"))
        .spawn()
        .context("spawning daemon process")?;

    // The server should start up and run without incident for
    // half a second.
    std::thread::sleep(time::Duration::from_millis(500));

    child.kill().context("killing child")?;

    let mut stdout = child.stdout.take().context("missing stdout")?;
    let mut stdout_str = String::from("");
    stdout.read_to_string(&mut stdout_str).context("slurping stdout")?;

    if stdout_str != "" {
        println!("{}", stdout_str);
        return Err(anyhow!("unexpected stdout output"));
    }

    let mut stderr = child.stderr.take().context("missing stderr")?;
    let mut stderr_str = String::from("");
    stderr.read_to_string(&mut stderr_str).context("slurping stderr")?;
    assert!(stderr_str.contains("STARTING DAEMON"));

    Ok(())
}
