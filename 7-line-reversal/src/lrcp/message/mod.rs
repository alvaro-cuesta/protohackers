mod parser;

use crate::SessionId;
use std::{fmt::Debug, io};
use thiserror::Error;

#[derive(PartialEq, Eq)]
pub(super) enum LrcpMessage {
    Connect {
        session: SessionId,
    },
    Data {
        session: SessionId,
        position: u32,
        data: Vec<u8>,
    },
    Ack {
        session: SessionId,
        length: u32,
    },
    Close {
        session: SessionId,
    },
}

impl Debug for LrcpMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect { session } => f.write_fmt(format_args!("/connect/{session}/")),
            Self::Data {
                session,
                position,
                data,
            } => {
                if let Ok(data) = std::str::from_utf8(&data) {
                    f.write_fmt(format_args!("/data/{session}/{position}/{data:?}/"))
                } else {
                    f.write_fmt(format_args!("/data/{session}/{position}/{data:?}/"))
                }
            }
            Self::Ack { session, length } => f.write_fmt(format_args!("/ack/{session}/{length}/")),
            Self::Close { session } => f.write_fmt(format_args!("/close/{session}/")),
        }
    }
}

impl LrcpMessage {
    pub(super) fn from<T: AsRef<[u8]>>(input: T) -> Result<LrcpMessage, LrcpMessageError> {
        let input = input.as_ref();
        let (_, message) = parser::parse_message(input).map_err(|_| LrcpMessageError::Parse)?;
        Ok(message)
    }

    // TODO: Make to_async_writer or similar?
    pub(super) fn to_vec(&self) -> Vec<u8> {
        match self {
            LrcpMessage::Connect { session } => format!("/connect/{session}/").into_bytes(),
            LrcpMessage::Data {
                session,
                position,
                data,
            } => {
                let data_bytes = data.iter().flat_map(|&x| {
                    if x == b'\\' {
                        vec![b'\\', b'\\']
                    } else if x == b'/' {
                        vec![b'\\', b'/']
                    } else {
                        vec![x]
                    }
                });

                format!("/data/{session}/{position}/")
                    .into_bytes()
                    .into_iter()
                    .chain(data_bytes)
                    .chain(std::iter::once(b'/'))
                    .collect()
            }
            LrcpMessage::Ack { session, length } => {
                format!("/ack/{session}/{length}/").into_bytes()
            }
            LrcpMessage::Close { session } => format!("/close/{session}/").into_bytes(),
        }
    }

    pub(super) fn session_id(&self) -> u32 {
        match self {
            LrcpMessage::Connect { session } => *session,
            LrcpMessage::Data { session, .. } => *session,
            LrcpMessage::Ack { session, .. } => *session,
            LrcpMessage::Close { session } => *session,
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum LrcpMessageError {
    #[error("error parsing message")]
    Parse,
    #[error("io error on codec: {0}")]
    Io(#[from] io::Error),
}

impl From<LrcpMessageError> for io::Error {
    fn from(error: LrcpMessageError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}
