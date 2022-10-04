use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::thread;

use anyhow::{anyhow, Context};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use crossbeam::channel;
use log::trace;
use serde_derive::{Serialize, Deserialize};

use super::consts;

/// ConnectHeader is the blob of metadata that a client transmits when it
/// first connections. It uses an enum to allow different connection types
/// to be initiated on the same socket. The ConnectHeader is always prefixed
/// with a 4 byte little endian unsigned word to indicate length.
#[derive(Serialize, Deserialize, Debug)]
pub enum ConnectHeader {
    Attach(AttachHeader),
    List,
}

/// AttachHeader is the blob of metadata that a client transmits when it
/// first dials into the shpool indicating which shell it wants to attach
/// to.
#[derive(Serialize, Deserialize, Debug)]
pub struct AttachHeader {
    /// The name of the session to create or attach to.
    pub name: String,
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

/// AttachStatus indicates what happened during an attach attempt.
#[derive(Serialize, Deserialize)]
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
}

/// FrameKind is a tag that indicates what type of frame is being transmitted
/// through the socket.
#[derive(Copy, Clone, Debug)]
pub enum ChunkKind {
    Stdout = 0,
    Stderr = 1,
    NoOp = 2,
}

impl ChunkKind {
    fn from_u8(v: u8) -> anyhow::Result<Self> {
        match v {
            0 => Ok(ChunkKind::Stdout),
            1 => Ok(ChunkKind::Stderr),
            2 => Ok(ChunkKind::NoOp),
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
    pub fn write_to<W>(&self, w: &mut W, stop_rx: channel::Receiver<()>) -> anyhow::Result<()>
        where W: std::io::Write
    {
        if let Ok(_) = stop_rx.try_recv() {
            return Ok(())
        }

        w.write_u8(self.kind as u8)?;

        if let Ok(_) = stop_rx.try_recv() {
            return Ok(())
        }

        w.write_u32::<LittleEndian>(self.buf.len() as u32)?;

        if let Ok(_) = stop_rx.try_recv() {
            return Ok(())
        }

        let mut to_write = &self.buf[..];
        while to_write.len() > 0 {
            if let Ok(_) = stop_rx.try_recv() {
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
                    return Err(e).context("reading stdout chunk");
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
    pub fn new(socket: PathBuf) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(socket).context("connecting to shpool")?;
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
}
