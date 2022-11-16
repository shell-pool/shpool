use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{io, thread};

use anyhow::{anyhow, Context};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use log::{info, debug, trace};
use serde_derive::{Serialize, Deserialize};

use super::{consts, tty};

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
    /// Take a global lock for 5s, waiting for a LocalCommandSetName
    /// to arrive to release the lock and inform us of the session to
    /// connect to.
    ///
    /// Responds with an AttachReplyHeader.
    RemoteCommandLock,
    /// Release a parked RemoteCommandLock thread.
    ///
    /// Responds with LocalCommandSetNameReply.
    LocalCommandSetName(LocalCommandSetNameRequest),
    /// List all of the currently active sessions.
    List,
}

/// AttachHeader is the blob of metadata that a client transmits when it
/// first dials into the shpool indicating which shell it wants to attach
/// to.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct AttachHeader {
    /// The name of the session to create or attach to.
    pub name: String,
    /// The value of the TERM environment variable in the client's
    /// shell. This needs to be forwarded so that the remote shell
    /// can interpret and emit control codes correctly.
    pub term: String,
    /// The size of the local tty. Passed along so that the remote
    /// pty can be kept in sync (important so curses applications look
    /// right).
    pub local_tty_size: tty::Size,
}

/// LocalCommandSetNameRequest releases the lock created by a ConnectHeader::RemoteCommandLock
/// informing the parked thread of the name of the session it should try to attach to.
#[derive(Serialize, Deserialize, Debug)]
pub struct LocalCommandSetNameRequest {
    /// The name of the session to create or attach to.
    pub name: String,
    /// The value of the local TERM environment variable.
    pub term: String,
    /// The size of the local tty.
    pub local_tty_size: tty::Size,
}

/// AttachReplyHeader is the blob of metadata that the shpool service prefixes
/// the data stream with after an attach. In can be used to indicate a connection
/// error.
#[derive(Serialize, Deserialize)]
pub struct AttachReplyHeader {
    pub status: AttachStatus,
}

/// ListReply is contains a list of active sessions to be displayed to the user.
#[derive(Serialize, Deserialize)]
pub struct ListReply {
    pub sessions: Vec<Session>,
}

/// Session describes an active session.
#[derive(Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    pub started_at_unix_ms: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LocalCommandSetNameReply {
    pub status: LocalCommandSetNameStatus,
}
#[derive(Serialize, Deserialize, Debug)]
pub enum LocalCommandSetNameStatus {
    /// Indicates we timed out waiting to link up with the remote command
    /// thread.
    Timeout,
    /// We successfully released the lock and allowed the attach
    /// process to proceed.
    Ok,
}

/// AttachStatus indicates what happened during an attach attempt.
#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachStatus {
    /// Attached indicates that there was an existing shell session with
    /// the given name, and `shpool attach` successfully connected to it.
    Attached,
    /// Created indicates that there was no existing shell session with the
    /// given name, so `shpool` created a new one.
    Created,
    /// Busy indicates that there is an existing shell session with the given
    /// name, but another `shpool attach` session is currently connected to
    /// it, so the connection attempt was rejected.
    Busy,
    /// Timeouted out waiting for a session to attach to. Only happens in
    /// response to a RemoteCommandLock style attach attempt.
    Timeout,
    /// Indicates that the parksing slot for an inbound ssh-extension style
    /// attach is occupied, and the user should try to reconnect again later.
    SshExtensionParkingSlotFull,
    /// Some unexpected error
    UnexpectedError(String),
}

/// FrameKind is a tag that indicates what type of frame is being transmitted
/// through the socket.
#[derive(Copy, Clone, Debug)]
pub enum ChunkKind {
    Data = 0,
    Heartbeat = 1,
}

impl ChunkKind {
    fn from_u8(v: u8) -> anyhow::Result<Self> {
        match v {
            0 => Ok(ChunkKind::Data),
            1 => Ok(ChunkKind::Heartbeat),
            _ => Err(anyhow!("unknown FrameKind {}", v)),
        }
    }
}

/// Chunk represents of a chunk of data meant for stdout or stderr.
/// Chunks get interleaved over the unix socket connection that
/// `shpool attach` uses to talk to `shpool daemon`, with the following
/// format:
///
/// ```
/// 1 byte: kind tag
/// little endian 4 byte word: length prefix
/// N bytes: data
/// ```
#[derive(Debug)]
pub struct Chunk<'data> {
    pub kind: ChunkKind,
    pub buf: &'data [u8],
}

impl<'data> Chunk<'data> {
    pub fn write_to<W>(&self, w: &mut W, stop: &AtomicBool) -> io::Result<()>
        where W: std::io::Write
    {
        if stop.load(Ordering::Relaxed) {
            return Ok(())
        }

        w.write_u8(self.kind as u8)?;

        if stop.load(Ordering::Relaxed) {
            return Ok(())
        }

        w.write_u32::<LittleEndian>(self.buf.len() as u32)?;

        if stop.load(Ordering::Relaxed) {
            return Ok(())
        }

        let mut to_write = &self.buf[..];
        while to_write.len() > 0 {
            if stop.load(Ordering::Relaxed) {
                return Ok(())
            }

            let nwritten = match w.write(&to_write) {
                Ok(n) => n,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        trace!("chunk: writing buffer: WouldBlock");
                        thread::sleep(consts::PIPE_POLL_DURATION);
                        continue;
                    }
                    return Err(e);
                }
            };
            to_write = &to_write[nwritten..];
        }

        Ok(())
    }

    pub fn read_into<R>(r: &mut R, buf: &'data mut [u8]) -> anyhow::Result<Self>
        where R: std::io::Read
    {
        let kind = r.read_u8()?;
        let len = r.read_u32::<LittleEndian>()? as usize;
        if len as usize > buf.len() {
            return Err(anyhow!("chunk of size {} exceeds size limit of {} bytes", len, buf.len()));
        }
        r.read_exact(&mut buf[..len])?;

        Ok(Chunk {
            kind: ChunkKind::from_u8(kind)?,
            buf: &buf[..len],
        })
    }
}

pub struct Client {
    pub stream: UnixStream,
}

impl Client {
    pub fn new(sock: PathBuf) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(sock).context("connecting to shpool")?;
        Ok(Client { stream })
    }

    pub fn write_connect_header(&mut self, header: ConnectHeader) -> anyhow::Result<()> {
        let buf = rmp_serde::to_vec(&header).context("formatting reply header")?;
        self.stream.write_u32::<LittleEndian>(buf.len() as u32)
            .context("writing reply length prefix")?;
        self.stream.write_all(&buf).context("writing reply header")?;

        Ok(())
    }

    pub fn read_reply<'data, R>(&mut self) -> anyhow::Result<R>
        where R: serde::de::DeserializeOwned
    {
        let length_prefix = self.stream.read_u32::<LittleEndian>()
            .context("reading header length prefix")?;
        let mut buf: Vec<u8> = vec![0; length_prefix as usize];
        self.stream.read_exact(&mut buf).context("reading header buf")?;

        let reply: R = rmp_serde::from_read(&*buf).context("parsing header")?;
        Ok(reply)
    }

    pub fn pipe_bytes(self) -> anyhow::Result<()> {
        let stop = AtomicBool::new(false);

        let mut read_client_stream = self.stream.try_clone().context("cloning read stream")?;
        let mut write_client_stream = self.stream.try_clone().context("cloning read stream")?;

        thread::scope(|s| {
            // stdin -> sock
            let stdin_to_sock_h = s.spawn(|| -> anyhow::Result<()> {
                info!("pipe_bytes: stdin->sock thread spawned");

                let mut stdin = std::io::stdin().lock();
                let mut buf = vec![0; consts::BUF_SIZE];

                nix::fcntl::fcntl(
                    stdin.as_raw_fd(),
                    nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
                ).context("setting stdin nonblocking")?;

                loop {
                    if stop.load(Ordering::Relaxed) {
                        info!("pipe_bytes: stdin->sock: recvd stop msg (1)");
                        return Ok(())
                    }

                    let nread = match stdin.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                trace!("pipe_bytes: stdin->sock: read: WouldBlock");
                                thread::sleep(consts::PIPE_POLL_DURATION);
                                continue;
                            }
                            return Err(e).context("reading stdin from user");
                        }
                    };

                    debug!("pipe_bytes: stdin->sock: read {} bytes", nread);

                    let mut to_write = &buf[..nread];
                    debug!("pipe_bytes: stdin->sock: created to_write='{}'", String::from_utf8_lossy(to_write));
                    while to_write.len() > 0 {
                        if stop.load(Ordering::Relaxed) {
                            info!("pipe_bytes: stdin->sock: recvd stop msg (2)");
                            return Ok(())
                        }

                        let nwritten = write_client_stream.write(to_write).context("writing chunk to server")?;
                        to_write = &to_write[nwritten..];
                        trace!("pipe_bytes: stdin->sock: to_write={}", String::from_utf8_lossy(to_write));
                    }

                    write_client_stream.flush().context("flushing client")?;
                }
            });

            // sock -> stdout
            let sock_to_stdout_h = s.spawn(|| -> anyhow::Result<()> {
                info!("pipe_bytes: sock->stdout thread spawned");

                let mut stdout = std::io::stdout().lock();
                let mut buf = vec![0; consts::BUF_SIZE];

                loop {
                    if stop.load(Ordering::Relaxed) {
                        info!("pipe_bytes: sock->stdout: recvd stop msg (1)");
                        return Ok(())
                    }

                    let chunk = Chunk::read_into(&mut read_client_stream, &mut buf)
                        .context("reading output chunk from daemon")?;

                    if chunk.buf.len() > 0 {
                        debug!("pipe_bytes: sock->stdout: chunk='{}' kind={:?} len={}",
                               String::from_utf8_lossy(chunk.buf), chunk.kind, chunk.buf.len());
                    }

                    let mut to_write = &chunk.buf[..];
                    match chunk.kind {
                        ChunkKind::Heartbeat => {
                            trace!("pipe_bytes: got heartbeat chunk");
                        },
                        ChunkKind::Data => {
                            while to_write.len() > 0  {
                                if stop.load(Ordering::Relaxed) {
                                    info!("pipe_bytes: sock->stdout: recvd stop msg (2)");
                                    return Ok(())
                                }

                                debug!("pipe_bytes: sock->stdout: about to select on stdout");
                                let mut stdout_set = nix::sys::select::FdSet::new();
                                stdout_set.insert(stdout.as_raw_fd());
                                let mut poll_dur = consts::PIPE_POLL_DURATION_TIMEVAL.clone();
                                let nready = nix::sys::select::select(
                                    None,
                                    None,
                                    Some(&mut stdout_set),
                                    None,
                                    Some(&mut poll_dur),
                                ).context("selecting on stdout")?;
                                if nready == 0 || !stdout_set.contains(stdout.as_raw_fd()) {
                                    continue;
                                }

                                let nwritten = stdout.write(to_write).context("writing chunk to stdout")?;
                                debug!("pipe_bytes: sock->stdout: wrote {} stdout bytes",
                                    nwritten);
                                to_write = &to_write[nwritten..];
                            }

                            stdout.flush().context("flushing stdout")?;
                            debug!("pipe_bytes: sock->stdout: flushed stdout");
                        },
                    }
                }
            });

            loop {
                if stdin_to_sock_h.is_finished() || sock_to_stdout_h.is_finished() {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
                thread::sleep(consts::JOIN_POLL_DURATION);
            }
            match stdin_to_sock_h.join() {
                Ok(v) => v?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }
            match sock_to_stdout_h.join() {
                Ok(v) => v?,
                Err(panic_err) => std::panic::resume_unwind(panic_err),
            }


            Ok(())
        })
    }
}
