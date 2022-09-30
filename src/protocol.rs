use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use anyhow::Context;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde_derive::{Serialize, Deserialize};

/// ConnectHeader is the blob of metadata that a client transmits when it
/// first connections. It uses an enum to allow different connection types
/// to be initiated on the same socket. The ConnectHeader is always prefixed
/// with a 4 byte little endian unsigned word to indicate length.
#[derive(Serialize, Deserialize)]
pub enum ConnectHeader {
    Attach(AttachHeader),
    List,
}

/// AttachHeader is the blob of metadata that a client transmits when it
/// first dials into the shpool indicating which shell it wants to attach
/// to.
#[derive(Serialize, Deserialize)]
pub struct AttachHeader {
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
