use bytes::{BufMut, BytesMut};

use crate::{HeartbeatInterval, Mile, Plate, Road, Speed, Timestamp};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MessageToClient {
    Error {
        msg: String,
    },
    Ticket {
        plate: Plate,
        road: Road,
        mile1: Mile,
        timestamp1: Timestamp,
        mile2: Mile,
        timestamp2: Timestamp,
        speed: Speed,
    },
    Heartbeat,
}

impl MessageToClient {
    pub fn error<T: AsRef<str>>(message: T) -> Self {
        MessageToClient::Error {
            msg: message.as_ref().to_owned(),
        }
    }

    pub fn into_bytes_mut(&self, dst: &mut BytesMut) {
        match self {
            MessageToClient::Error { msg } => {
                let msg = msg.as_bytes();
                assert!(msg.len() <= 0xFF);

                dst.reserve(1 + 1 + msg.len());

                dst.put_u8(0x10);
                dst.put_u8(msg.len() as u8);
                dst.extend_from_slice(msg);
            }
            MessageToClient::Ticket {
                plate,
                road,
                mile1,
                timestamp1,
                mile2,
                timestamp2,
                speed,
            } => {
                let plate = plate.as_bytes();
                assert!(plate.len() <= 0xFF);

                dst.reserve(1 + 1 + plate.len() + 2 + 2 + 4 + 2 + 4 + 2);

                dst.put_u8(0x21);
                dst.put_u8(plate.len() as u8);
                dst.extend_from_slice(plate);
                dst.put_u16(*road);
                dst.put_u16(*mile1);
                dst.put_u32(*timestamp1);
                dst.put_u16(*mile2);
                dst.put_u32(*timestamp2);
                dst.put_u16(*speed);
            }
            MessageToClient::Heartbeat => {
                dst.put_u8(0x41);
            }
        }
    }

    pub fn is_error(&self) -> bool {
        match self {
            Self::Error { .. } => true,
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MessageToServer {
    Plate {
        plate: Plate,
        timestamp: Timestamp,
    },
    WantHeartbeat {
        interval: HeartbeatInterval,
    },
    IAmCamera {
        road: Road,
        mile: Mile,
        limit: Speed,
    },
    IAmDispatcher {
        roads: Vec<Road>,
    },
}
