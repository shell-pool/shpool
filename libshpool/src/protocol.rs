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
    fmt,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    sync::atomic::{AtomicI32, Ordering},
    thread, time,
};

use anyhow::{anyhow, Context};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde_derive::{Deserialize, Serialize};
use tracing::{debug, error, instrument, span, trace, warn, Level};

use super::{consts, tty};

const JOIN_POLL_DUR: time::Duration = time::Duration::from_millis(100);
const JOIN_HANGUP_DUR: time::Duration = time::Duration::from_millis(300);

/// ConnectHeader is the blob of metadata that a client transmits when it
/// first connections. It uses an enum to allow different connection types
/// to be initiated on the same socket. The ConnectHeader is always prefixed
/// with a 4 byte little endian unsigned word to indicate length.
#[derive(Serialize, Deserialize, Debug)]
pub enum ConnectHeader {
    /// Attach to the named session indicated by the given header.
    ///
    /// Responds with an AttachReplyHeader.
    Attach(AttachHeader),
    /// List all of the currently active sessions.
    List,
    /// A message for a named, running sessions. This
    /// provides a mechanism for RPC-like calls to be
    /// made to running sessions. Messages are only
    /// delivered if there is currently a client attached
    /// to the session because we need a servicing thread
    /// with access to the SessionInner to respond to requests
    /// (we could implement a mailbox system or something
    /// for detached threads, but so far we have not needed to).
    SessionMessage(SessionMessageRequest),
    /// A message to request that a list of running
    /// sessions get detached from.
    Detach(DetachRequest),
    /// A message to request that a list of running
    /// sessions get killed.
    Kill(KillRequest),
}

/// KillRequest represents a request to kill
/// the given named sessions.
#[derive(Serialize, Deserialize, Debug)]
pub struct KillRequest {
    /// The sessions to detach
    pub sessions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KillReply {
    pub not_found_sessions: Vec<String>,
}

/// DetachRequest represents a request to detach
/// from the given named sessions.
#[derive(Serialize, Deserialize, Debug)]
pub struct DetachRequest {
    /// The sessions to detach
    pub sessions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DetachReply {
    /// sessions that are not even in the session table
    pub not_found_sessions: Vec<String>,
    /// sessions that are in the session table, but have no
    /// tty attached
    pub not_attached_sessions: Vec<String>,
}

/// SessionMessageRequest represents a request that
/// ought to be routed to the session indicated by
/// `session_name`.
#[derive(Serialize, Deserialize, Debug)]
pub struct SessionMessageRequest {
    /// The session to route this request to.
    pub session_name: String,
    /// The actual message to send to the session.
    pub payload: SessionMessageRequestPayload,
}

/// SessionMessageRequestPayload contains a request for
/// a running session.
#[derive(Serialize, Deserialize, Debug)]
pub enum SessionMessageRequestPayload {
    /// Resize a named session's pty. Generated when
    /// a `shpool attach` process receives a SIGWINCH.
    Resize(ResizeRequest),
    /// Detach the given session. Generated internally
    /// by the server from a batch detach request.
    Detach,
}

/// ResizeRequest resizes the pty for a given named session.
/// We use an out-of-band request rather than doing this
/// in the input stream because we don't want to have to
/// introduce a framing protocol for the input stream.
#[derive(Serialize, Deserialize, Debug)]
pub struct ResizeRequest {
    /// The size of the client's tty
    pub tty_size: tty::Size,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum SessionMessageReply {
    /// The session was not found in the session table
    NotFound,
    /// There is not terminal attached to the session so
    /// it can't handle messages right now.
    NotAttached,
    /// The response to a resize message
    Resize(ResizeReply),
    /// The response to a detach message
    Detach(SessionMessageDetachReply),
}

/// A reply to a detach message
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum SessionMessageDetachReply {
    Ok,
}

/// A reply to a resize message
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ResizeReply {
    Ok,
}

/// AttachHeader is the blob of metadata that a client transmits when it
/// first dials into the shpool daemon indicating which shell it wants
/// to attach to.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct AttachHeader {
    /// The name of the session to create or attach to.
    pub name: String,
    /// The size of the local tty. Passed along so that the remote
    /// pty can be kept in sync (important so curses applications look
    /// right).
    pub local_tty_size: tty::Size,
    /// A subset of the environment of the shell that `shpool attach` is run
    /// in. Contains only some variables needed to set up the shell when
    /// shpool forks off a process. For now the list is just `SSH_AUTH_SOCK`
    /// and `TERM`.
    pub local_env: Vec<(String, String)>,
    /// If specified, sets a time limit on how long the shell will be open
    /// when the shell is first created (does nothing in the case of a
    /// reattach). The daemon is responsible for automatically killing the
    /// session once the ttl is over.
    pub ttl_secs: Option<u64>,
    /// If specified, a command to run instead of the users default shell.
    pub cmd: Option<String>,
}

impl AttachHeader {
    pub fn local_env_get(&self, var: &str) -> Option<&str> {
        self.local_env.iter().find(|(k, _)| k == var).map(|(_, v)| v.as_str())
    }
}

/// AttachReplyHeader is the blob of metadata that the shpool service prefixes
/// the data stream with after an attach. In can be used to indicate a
/// connection error.
#[derive(Serialize, Deserialize, Debug)]
pub struct AttachReplyHeader {
    pub status: AttachStatus,
}

/// ListReply is contains a list of active sessions to be displayed to the user.
#[derive(Serialize, Deserialize, Debug)]
pub struct ListReply {
    pub sessions: Vec<Session>,
}

/// Session describes an active session.
#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    pub name: String,
    pub started_at_unix_ms: i64,
    pub status: SessionStatus,
}

/// Indicates if a shpool session currently has a client attached.
#[derive(Serialize, Deserialize, Debug)]
pub enum SessionStatus {
    Attached,
    Disconnected,
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionStatus::Attached => write!(f, "attached"),
            SessionStatus::Disconnected => write!(f, "disconnected"),
        }
    }
}

/// AttachStatus indicates what happened during an attach attempt.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub enum AttachStatus {
    /// Attached indicates that there was an existing shell session with
    /// the given name, and `shpool attach` successfully connected to it.
    ///
    /// NOTE: warnings is not currently used, but it used to be, and we
    /// might want it in the future, so it is not worth breaking the protocol
    /// over.
    Attached { warnings: Vec<String> },
    /// Created indicates that there was no existing shell session with the
    /// given name, so `shpool` created a new one.
    ///
    /// NOTE: warnings is not currently used, see above.
    Created { warnings: Vec<String> },
    /// Busy indicates that there is an existing shell session with the given
    /// name, but another shpool session is currently connected to
    /// it, so the connection attempt was rejected.
    Busy,
    /// Forbidden indicates that the daemon has rejected the connection
    /// attempt for security reasons.
    Forbidden(String),
    /// Some unexpected error
    UnexpectedError(String),
}

/// ChunkKind is a tag that indicates what type of frame is being transmitted
/// through the socket.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ChunkKind {
    /// After the kind tag, the chunk will have a 4 byte little endian length
    /// prefix followed by the actual data.
    Data = 0,
    /// An empty chunk sent so that the daemon can check to make sure the attach
    /// process is still listening.
    Heartbeat = 1,
    /// The child shell has exited. After the kind tag, the chunk will
    /// have exactly 4 bytes of data, which will contain a little endian
    /// code indicating the child's exit status.
    ExitStatus = 2,
}

impl TryFrom<u8> for ChunkKind {
    type Error = anyhow::Error;

    fn try_from(v: u8) -> anyhow::Result<Self> {
        match v {
            0 => Ok(ChunkKind::Data),
            1 => Ok(ChunkKind::Heartbeat),
            2 => Ok(ChunkKind::ExitStatus),
            _ => Err(anyhow!("unknown ChunkKind {}", v)),
        }
    }
}

/// Chunk represents of a chunk of data in the output stream
///
/// format:
///
/// ```text
/// 1 byte: kind tag
/// little endian 4 byte word: length prefix
/// N bytes: data
/// ```
#[derive(Debug, PartialEq)]
pub struct Chunk<'data> {
    pub kind: ChunkKind,
    pub buf: &'data [u8],
}

impl<'data> Chunk<'data> {
    pub fn write_to<W>(&self, w: &mut W) -> io::Result<()>
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

    pub fn read_into<R>(r: &mut R, buf: &'data mut [u8]) -> anyhow::Result<Self>
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
    pub stream: UnixStream,
}

impl Client {
    pub fn new<P: AsRef<Path>>(sock: P) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(sock).context("connecting to shpool")?;
        Ok(Client { stream })
    }

    pub fn write_connect_header(&mut self, header: ConnectHeader) -> anyhow::Result<()> {
        let serialize_stream = self.stream.try_clone().context("cloning stream for reply")?;
        bincode::serialize_into(serialize_stream, &header).context("writing reply")?;

        Ok(())
    }

    pub fn read_reply<R>(&mut self) -> anyhow::Result<R>
    where
        R: serde::de::DeserializeOwned,
    {
        let reply: R = bincode::deserialize_from(&mut self.stream).context("parsing header")?;
        Ok(reply)
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
}
