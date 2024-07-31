// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    cmp,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    sync::atomic::{AtomicI32, Ordering},
    thread, time,
};

use anyhow::{anyhow, Context};
use byteorder::{LittleEndian, ReadBytesExt as _, WriteBytesExt as _};
use serde::{Deserialize, Serialize};
use shpool_protocol::{Chunk, ChunkKind, ConnectHeader, VersionHeader};
use tracing::{debug, error, info, instrument, span, trace, warn, Level};

use super::{consts, tty};

const JOIN_POLL_DUR: time::Duration = time::Duration::from_millis(100);
const JOIN_HANGUP_DUR: time::Duration = time::Duration::from_millis(300);

/// The centralized encoding function that should be used for all protocol
/// serialization.
pub fn encode_to<T, W>(d: &T, w: W) -> anyhow::Result<()>
where
    T: Serialize,
    W: Write,
{
    // You might be worried that since we are encoding and decoding
    // directly to/from the stream, unknown fields might be left trailing
    // and mangle followup data, but msgpack is basically binary
    // encoded json, so it has a notion of an object, which means
    // it will be able to skip past the unknown fields and avoid any
    // sort of mangling.
    let mut serializer = rmp_serde::Serializer::new(w).with_struct_map();
    d.serialize(&mut serializer).context("serializing data")?;
    Ok(())
}

/// The centralized decoding focuntion that should be used for all protocol
/// deserialization.
pub fn decode_from<T, R>(r: R) -> anyhow::Result<T>
where
    for<'de> T: Deserialize<'de>,
    R: Read,
{
    let mut deserializer = rmp_serde::Deserializer::new(r);
    let d: T = Deserialize::deserialize(&mut deserializer).context("deserializing from reader")?;
    Ok(d)
}

/// Methods for the Chunk protocol struct. Protocol structs
/// are always bare structs, so we use ext traits to mix in methods.
pub trait ChunkExt<'data>: Sized {
    fn write_to<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: std::io::Write;

    fn read_into<R>(r: &mut R, buf: &'data mut [u8]) -> anyhow::Result<Self>
    where
        R: std::io::Read;
}

impl<'data> ChunkExt<'data> for Chunk<'data> {
    fn write_to<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: std::io::Write,
    {
        w.write_u8(self.kind as u8)?;
        if let ChunkKind::ExitStatus = self.kind {
            assert!(self.buf.len() == 4);
            // the caller should have already little-endian encoded
            // the exit status and stuffed it into buf
        } else {
            w.write_u32::<LittleEndian>(self.buf.len() as u32)?;
        }
        w.write_all(self.buf)?;

        Ok(())
    }

    fn read_into<R>(r: &mut R, buf: &'data mut [u8]) -> anyhow::Result<Self>
    where
        R: std::io::Read,
    {
        let kind = r.read_u8()?;
        let kind = ChunkKind::try_from(kind)?;
        if let ChunkKind::ExitStatus = kind {
            if 4 > buf.len() {
                return Err(anyhow!("chunk of size 4 exceeds size limit of {} bytes", buf.len()));
            }

            r.read_exact(&mut buf[..4])?;
            Ok(Chunk { kind, buf: &buf[..4] })
        } else {
            let len = r.read_u32::<LittleEndian>()? as usize;
            if len > buf.len() {
                return Err(anyhow!(
                    "chunk of size {} exceeds size limit of {} bytes",
                    len,
                    buf.len()
                ));
            }
            r.read_exact(&mut buf[..len])?;
            Ok(Chunk { kind, buf: &buf[..len] })
        }
    }
}

pub struct Client {
    stream: UnixStream,
}

/// The result of creating a client, possibly with
/// flagging some issues that need to be handled.
pub enum ClientResult {
    /// The created client, ready to go.
    JustClient(Client),
    /// There was a version mismatch between the client
    /// process and the daemon process which ought to be
    /// handled, though it is possible that some operations
    /// will continue to work.
    VersionMismatch {
        /// A warning about a version mismatch that should be
        /// displayed to the user.
        warning: String,
        /// The client, which may or may not work.
        client: Client,
    },
}

impl Client {
    /// Create a new client
    #[allow(clippy::new_ret_no_self)]
    pub fn new<P: AsRef<Path>>(sock: P) -> anyhow::Result<ClientResult> {
        let stream = UnixStream::connect(sock).context("connecting to shpool")?;

        let daemon_version: VersionHeader = match decode_from(&stream) {
            Ok(v) => v,
            Err(e) => {
                warn!("error parsing VersionHeader: {:?}", e);
                return Ok(ClientResult::VersionMismatch {
                    warning: String::from("could not get daemon version"),
                    client: Client { stream },
                });
            }
        };
        info!("read daemon version header: {:?}", daemon_version);

        match Self::version_ord(shpool_protocol::VERSION, &daemon_version.version)
            .context("comparing versions")?
        {
            cmp::Ordering::Equal => Ok(ClientResult::JustClient(Client { stream })),
            cmp::Ordering::Less => Ok(ClientResult::VersionMismatch {
                warning: format!(
                    "client protocol (version {:?}) is older than daemon protocol (version {:?})",
                    shpool_protocol::VERSION,
                    daemon_version.version,
                ),
                client: Client { stream },
            }),
            cmp::Ordering::Greater => Ok(ClientResult::VersionMismatch {
                warning: format!(
                    "client protocol ({:?}) is newer than daemon protocol (version {:?})",
                    shpool_protocol::VERSION,
                    daemon_version.version,
                ),
                client: Client { stream },
            }),
        }
    }

    pub fn write_connect_header(&self, header: ConnectHeader) -> anyhow::Result<()> {
        encode_to(&header, &self.stream).context("writing reply")?;
        Ok(())
    }

    pub fn read_reply<R>(&mut self) -> anyhow::Result<R>
    where
        R: for<'de> serde::Deserialize<'de>,
    {
        let reply: R = decode_from(&mut self.stream).context("parsing header")?;
        Ok(reply)
    }

    /// This is essentially just PartialOrd on client version strings
    /// with more descriptive errors (since PartialOrd gives an option)
    /// and without having to wrap in a newtype.
    fn version_ord(client_version: &str, daemon_version: &str) -> anyhow::Result<cmp::Ordering> {
        let client_parts = client_version
            .split('.')
            .map(|p| p.parse::<i64>())
            .collect::<Result<Vec<_>, _>>()
            .context("parsing client version")?;
        if client_parts.len() != 3 {
            return Err(anyhow!(
                "parsing client version: got {} parts, want 3",
                client_parts.len(),
            ));
        }

        let daemon_parts = daemon_version
            .split('.')
            .map(|p| p.parse::<i64>())
            .collect::<Result<Vec<_>, _>>()
            .context("parsing daemon version")?;
        if daemon_parts.len() != 3 {
            return Err(anyhow!(
                "parsing daemon version: got {} parts, want 3",
                daemon_parts.len(),
            ));
        }

        // pre 1.0 releases flag breaking changes with their
        // minor version rather than major version.
        if client_parts[0] == 0 && daemon_parts[0] == 0 {
            return Ok(client_parts[1].cmp(&daemon_parts[1]));
        }

        Ok(client_parts[0].cmp(&daemon_parts[0]))
    }

    /// pipe_bytes suffles bytes from std{in,out} to the unix
    /// socket and back again. It is the main loop of
    /// `shpool attach`.
    ///
    /// Return value: the exit status that `shpool attach` should
    /// exit with.
    #[instrument(skip_all)]
    pub fn pipe_bytes(self) -> anyhow::Result<i32> {
        let tty_guard = tty::set_attach_flags()?;

        let mut read_client_stream = self.stream.try_clone().context("cloning read stream")?;
        let mut write_client_stream = self.stream.try_clone().context("cloning read stream")?;

        let exit_status = AtomicI32::new(1);
        thread::scope(|s| {
            // stdin -> sock
            let stdin_to_sock_h = s.spawn(|| -> anyhow::Result<()> {
                let _s = span!(Level::INFO, "stdin->sock").entered();
                let mut stdin = std::io::stdin().lock();
                let mut buf = vec![0; consts::BUF_SIZE];

                loop {
                    let nread = stdin.read(&mut buf).context("reading stdin from user")?;
                    if nread == 0 {
                        continue;
                    }
                    debug!("read {} bytes", nread);

                    let to_write = &buf[..nread];
                    trace!("created to_write='{}'", String::from_utf8_lossy(to_write));

                    write_client_stream.write_all(to_write)?;
                    write_client_stream.flush().context("flushing client")?;
                }
            });

            // sock -> stdout
            let sock_to_stdout_h = s.spawn(|| -> anyhow::Result<()> {
                let _s = span!(Level::INFO, "sock->stdout").entered();

                let mut stdout = std::io::stdout().lock();
                let mut buf = vec![0; consts::BUF_SIZE];

                loop {
                    let chunk = match Chunk::read_into(&mut read_client_stream, &mut buf) {
                        Ok(c) => c,
                        Err(err) => {
                            error!("reading chunk: {:?}", err);
                            return Err(err);
                        }
                    };

                    if !chunk.buf.is_empty() {
                        debug!(
                            "chunk='{}' kind={:?} len={}",
                            String::from_utf8_lossy(chunk.buf),
                            chunk.kind,
                            chunk.buf.len()
                        );
                    }

                    match chunk.kind {
                        ChunkKind::Heartbeat => {
                            trace!("got heartbeat chunk");
                        }
                        ChunkKind::Data => {
                            stdout.write_all(chunk.buf).context("writing chunk to stdout")?;

                            if let Err(e) = stdout.flush() {
                                if e.kind() == std::io::ErrorKind::WouldBlock {
                                    // If the fd is busy, we are likely just getting
                                    // flooded with output and don't need to worry about
                                    // flushing every last byte. Flushing is really
                                    // about interactive situations where we want to
                                    // see echoed bytes immediately.
                                    continue;
                                }
                            }
                            debug!("flushed stdout");
                        }
                        ChunkKind::ExitStatus => {
                            let mut status_reader = io::Cursor::new(chunk.buf);
                            exit_status.store(
                                status_reader
                                    .read_i32::<LittleEndian>()
                                    .context("reading exit status from exit status chunk")?,
                                Ordering::Release,
                            );
                        }
                    }
                }
            });

            loop {
                let mut nfinished_threads = 0;
                if stdin_to_sock_h.is_finished() {
                    nfinished_threads += 1;
                }
                if sock_to_stdout_h.is_finished() {
                    nfinished_threads += 1;
                }
                if nfinished_threads > 0 {
                    if nfinished_threads < 2 {
                        thread::sleep(JOIN_HANGUP_DUR);
                        nfinished_threads = 0;
                        if stdin_to_sock_h.is_finished() {
                            nfinished_threads += 1;
                        }
                        if sock_to_stdout_h.is_finished() {
                            nfinished_threads += 1;
                        }
                        if nfinished_threads < 2 {
                            // If one of the worker threads is done and the
                            // other is not exiting, we are likely blocked on
                            // some IO. Fortunately, since there isn't much else
                            // going on in the client process and the thing to do
                            // is to shut down at this point, we can resolve this
                            // by just hard-exiting the whole process. This allows
                            // us to use simple blocking IO.
                            warn!(
                                "exiting due to a stuck IO thread stdin_to_sock_finished={} sock_to_stdout_finished={}",
                                stdin_to_sock_h.is_finished(),
                                sock_to_stdout_h.is_finished()
                            );
                            // make sure that we restore the tty flags on the input
                            // tty before exiting the process.
                            drop(tty_guard);

                            std::process::exit(exit_status.load(Ordering::Acquire));
                        }
                    }
                    break;
                }
                thread::sleep(JOIN_POLL_DUR);
            }

            match stdin_to_sock_h.join() {
                Ok(v) => v?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }
            match sock_to_stdout_h.join() {
                Ok(v) => v?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }

            Ok(exit_status.load(Ordering::Acquire))
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn chunk_round_trip() {
        let data: Vec<u8> = vec![0, 0, 0, 1, 5, 6];
        let cases = vec![
            Chunk { kind: ChunkKind::Data, buf: data.as_slice() },
            Chunk { kind: ChunkKind::Heartbeat, buf: &data[..0] },
            Chunk { kind: ChunkKind::ExitStatus, buf: &data[..4] },
        ];

        let mut buf = vec![0; 256];
        for c in cases {
            let mut file_obj = io::Cursor::new(vec![0; 256]);
            c.write_to(&mut file_obj).expect("write to suceed");
            file_obj.set_position(0);
            let round_tripped =
                Chunk::read_into(&mut file_obj, &mut buf).expect("parse to succeed");
            assert_eq!(c, round_tripped);
        }
    }

    #[test]
    fn version_ordering_noerr() {
        use std::cmp::Ordering;

        let cases = vec![
            ("1.0.0", "1.0.0", Ordering::Equal),
            ("1.0.0", "1.0.1", Ordering::Equal),
            ("1.0.0", "1.1.0", Ordering::Equal),
            ("1.0.0", "1.1.1", Ordering::Equal),
            ("1.0.0", "1.100.100", Ordering::Equal),
            ("1.0.0", "2.0.0", Ordering::Less),
            ("1.0.0", "2.8.0", Ordering::Less),
            ("1.199.0", "2.8.0", Ordering::Less),
            ("2.0.0", "1.0.0", Ordering::Greater),
            ("0.1.0", "0.1.0", Ordering::Equal),
            ("0.1.1", "0.1.0", Ordering::Equal),
            ("0.1.1", "0.1.99", Ordering::Equal),
            ("0.1.0", "0.2.0", Ordering::Less),
            ("0.1.99", "0.2.0", Ordering::Less),
            ("0.2.0", "0.1.0", Ordering::Greater),
        ];

        for (lhs, rhs, ordering) in cases {
            let actual_ordering =
                Client::version_ord(lhs, rhs).expect("version strings to have an ordering");
            assert_eq!(actual_ordering, ordering);
        }
    }

    #[test]
    fn version_ordering_err() {
        let cases = vec![
            ("1.0.0", "1.0.0.0", "got 4 parts, want 3"),
            ("1.0.0.0", "1.0.0", "got 4 parts, want 3"),
            ("foobar", "1.0.0", "invalid digit found in string"),
            ("1.foobar", "1.0.0", "invalid digit found in string"),
        ];

        for (lhs, rhs, err_substr) in cases {
            if let Err(e) = Client::version_ord(lhs, rhs) {
                eprintln!("ERR: {:?}", e);
                eprintln!("EXPECTED SUBSTR: {}", err_substr);
                let errstr = format!("{:?}", e);
                assert!(errstr.contains(err_substr));
            } else {
                panic!("no error though we expected one");
            }
        }
    }
}
