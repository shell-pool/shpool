// Copyright 2024 Google LLC
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

use std::{default::Default, fmt};

use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The header used to advertize daemon version.
///
/// This header gets written by the daemon to every stream as
/// soon as it is opened, which allows the client to compare
/// version strings for protocol negotiation (basically just
/// deciding if the user ought to be warned about mismatched
/// versions).
#[derive(Serialize, Deserialize, Debug)]
pub struct VersionHeader {
    pub version: String,
}

/// The blob of metadata that a client transmits when it
/// first connects.
///
/// It uses an enum to allow different connection types
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
    #[serde(default)]
    pub sessions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KillReply {
    #[serde(default)]
    pub not_found_sessions: Vec<String>,
}

/// DetachRequest represents a request to detach
/// from the given named sessions.
#[derive(Serialize, Deserialize, Debug)]
pub struct DetachRequest {
    /// The sessions to detach
    #[serde(default)]
    pub sessions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DetachReply {
    /// sessions that are not even in the session table
    #[serde(default)]
    pub not_found_sessions: Vec<String>,
    /// sessions that are in the session table, but have no
    /// tty attached
    #[serde(default)]
    pub not_attached_sessions: Vec<String>,
}

/// SessionMessageRequest represents a request that
/// ought to be routed to the session indicated by
/// `session_name`.
#[derive(Serialize, Deserialize, Debug)]
pub struct SessionMessageRequest {
    /// The session to route this request to.
    #[serde(default)]
    pub session_name: String,
    /// The actual message to send to the session.
    #[serde(default)]
    pub payload: SessionMessageRequestPayload,
}

/// SessionMessageRequestPayload contains a request for
/// a running session.
#[derive(Serialize, Deserialize, Debug, Default)]
pub enum SessionMessageRequestPayload {
    /// Resize a named session's pty. Generated when
    /// a `shpool attach` process receives a SIGWINCH.
    Resize(ResizeRequest),
    /// Detach the given session. Generated internally
    /// by the server from a batch detach request.
    #[default]
    Detach,
}

/// ResizeRequest resizes the pty for a named session.
///
/// We use an out-of-band request rather than doing this
/// in the input stream because we don't want to have to
/// introduce a framing protocol for the input stream.
#[derive(Serialize, Deserialize, Debug)]
pub struct ResizeRequest {
    /// The size of the client's tty
    #[serde(default)]
    pub tty_size: TtySize,
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
    #[serde(default)]
    pub name: String,
    /// The size of the local tty. Passed along so that the remote
    /// pty can be kept in sync (important so curses applications look
    /// right).
    #[serde(default)]
    pub local_tty_size: TtySize,
    /// A subset of the environment of the shell that `shpool attach` is run
    /// in. Contains only some variables needed to set up the shell when
    /// shpool forks off a process. For now the list is just `SSH_AUTH_SOCK`
    /// and `TERM`.
    #[serde(default)]
    pub local_env: Vec<(String, String)>,
    /// If specified, sets a time limit on how long the shell will be open
    /// when the shell is first created (does nothing in the case of a
    /// reattach). The daemon is responsible for automatically killing the
    /// session once the ttl is over.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
    /// If specified, a command to run instead of the users default shell.
    #[serde(default)]
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
    #[serde(default)]
    pub status: AttachStatus,
}

/// ListReply is contains a list of active sessions to be displayed to the user.
#[derive(Serialize, Deserialize, Debug)]
pub struct ListReply {
    #[serde(default)]
    pub sessions: Vec<Session>,
}

/// Session describes an active session.
#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub started_at_unix_ms: i64,
    #[serde(default)]
    pub status: SessionStatus,
}

/// Indicates if a shpool session currently has a client attached.
#[derive(Serialize, Deserialize, Debug, Default)]
pub enum SessionStatus {
    #[default]
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

impl Default for AttachStatus {
    fn default() -> Self {
        AttachStatus::UnexpectedError(String::from("default"))
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct TtySize {
    pub rows: u16,
    pub cols: u16,
    pub xpixel: u16,
    pub ypixel: u16,
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
