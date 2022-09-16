use serde_derive::{Serialize, Deserialize};

/// AttachHeader is the blob of metadata that a client transmits when it
/// first dials into the shpool indicating which shell it wants to attach
/// to. The wire protocol for an AttachHeader is a 4 byte unsigned little
/// endian length prefix followed by a MsgPack encoded version of this
/// struct.
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
