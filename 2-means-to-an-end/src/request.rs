use thiserror::Error;

use crate::{Price, Timestamp};

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("received bad type tag: {0}")]
    BadType(u8),
}

#[derive(Debug)]
pub enum Request {
    Insert {
        timestamp: Timestamp,
        price: Price,
    },
    Query {
        mintime: Timestamp,
        maxtime: Timestamp,
    },
}

impl Request {
    pub const SIZE: usize = 9;
}

impl TryFrom<[u8; Request::SIZE]> for Request {
    type Error = RequestError;

    fn try_from(value: [u8; Request::SIZE]) -> Result<Self, Self::Error> {
        let typ = value[0];
        let field_1 = i32::from_be_bytes([value[1], value[2], value[3], value[4]]);
        let field_2 = i32::from_be_bytes([value[5], value[6], value[7], value[8]]);

        match typ {
            b'I' => Ok(Request::Insert {
                timestamp: field_1,
                price: field_2,
            }),
            b'Q' => Ok(Request::Query {
                mintime: field_1,
                maxtime: field_2,
            }),
            _ => Err(RequestError::BadType(typ)),
        }
    }
}
