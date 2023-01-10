mod message;
mod session_write;
mod socket;
mod state;

use self::session_write::LrcpSessionWrite;
use crate::SessionId;
use bytes::Bytes;
use std::{fmt::Debug, io, net::SocketAddr};
use thiserror::Error;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::io::StreamReader;

pub use socket::LrcpSocket;

#[derive(Debug, Error)]
pub enum LrcpSessionError {
    // TODO: This should probably not be an error and just EOF the stream?
    #[error("peer closed session")]
    PeerClosed,
    #[error("peer misbehaved")]
    PeerMisbehaved,
}

impl From<LrcpSessionError> for io::Error {
    fn from(err: LrcpSessionError) -> Self {
        match err {
            LrcpSessionError::PeerClosed => io::Error::new(io::ErrorKind::ConnectionReset, err),
            LrcpSessionError::PeerMisbehaved => io::Error::new(io::ErrorKind::Other, err),
        }
    }
}

pub type LrcpSessionReadItem = Result<Bytes, LrcpSessionError>;

pub type LrcpSessionRecv = StreamReader<UnboundedReceiverStream<LrcpSessionReadItem>, Bytes>;

pub type LrcpSessionItem = (LrcpSessionHandle, LrcpSessionRecv, LrcpSessionWrite);

pub const MAX_PACKET_SIZE: usize = 1000;

// TODO: This could be dynamic based on session id length but..
pub const MAX_DATA_SIZE: usize = MAX_PACKET_SIZE
    - (b"/data/".len()
        + b"4294967295".len()
        + b"/".len()
        + b"4294967295".len()
        + b"/".len()
        + b"/".len());

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct LrcpSessionHandle(SocketAddr, SessionId);

impl Debug for LrcpSessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let LrcpSessionHandle(addr, session_id) = self;

        f.write_fmt(format_args!("{addr}:{session_id}"))
    }
}
