use crate::{
    message::{MessageToClient, MessageToServer},
    parse::parse_message_to_server,
};
use bytes::{Buf, BytesMut};
use nom::Err;
use std::io;
use tokio_util::codec::{Decoder, Encoder};

pub struct MessageToServerDecoder;

impl Decoder for MessageToServerDecoder {
    type Item = MessageToServer;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }

        match parse_message_to_server(&src) {
            Ok((rest, val)) => {
                src.advance(src.len() - rest.len());
                Ok(Some(val))
            }
            Err(Err::Incomplete(_)) => Ok(None),
            Err(Err::Error(err)) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Recoverable error: {err:?}"),
            )),
            Err(Err::Failure(err)) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Unrecoverable error: {err:?}"),
            )),
        }
    }
}

pub struct MessageToClientEncoder;

impl Encoder<MessageToClient> for MessageToClientEncoder {
    type Error = io::Error;

    fn encode(&mut self, item: MessageToClient, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.into_bytes_mut(dst);

        #[cfg(debug_assertions)]
        {
            println!("{:?}", dst);
        }

        Ok(())
    }
}
