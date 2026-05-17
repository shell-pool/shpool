//! The `events` subcommand: connect to the daemon's events socket and
//! stream each line to stdout.
//!
//! The daemon-side protocol and fan-out machinery live in
//! `crate::daemon::events`.

use std::{
    io::{self, BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
};

use anyhow::Context;

/// Connect to the events socket, copy each line to stdout, and flush per
/// line so the stream is usable in pipes (`shpool events | jq`). Returns
/// when the daemon closes the connection.
pub fn run(socket_path: &Path) -> anyhow::Result<()> {
    let stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connecting to events socket {:?}", socket_path))?;
    let reader = BufReader::new(stream);
    let mut stdout = io::stdout().lock();
    for line in reader.lines() {
        let line = line.context("reading event")?;
        stdout.write_all(line.as_bytes()).context("writing event")?;
        stdout.write_all(b"\n").context("writing event newline")?;
        stdout.flush().context("flushing stdout")?;
    }
    Ok(())
}
