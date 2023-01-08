use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder, LinesCodec, LinesCodecError};

#[derive(Debug, Error)]
pub enum JsonCodecError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("lines codec error: {0}")]
    LinesCodec(#[from] LinesCodecError),
    #[error("de/serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

pub struct JsonCodec<Dec, Enc> {
    // TODO: Allow any inner codec ?
    lines_codec: LinesCodec,
    dec: PhantomData<Dec>,
    enc: PhantomData<Enc>,
}

impl<Dec, Enc> JsonCodec<Dec, Enc> {
    pub fn new() -> Self {
        Self {
            lines_codec: LinesCodec::new(),
            enc: PhantomData::default(),
            dec: PhantomData::default(),
        }
    }
}

impl<Dec, Enc> Decoder for JsonCodec<Dec, Enc>
where
    Dec: for<'de> Deserialize<'de>,
{
    type Item = Dec;
    type Error = JsonCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let Some(line) = self.lines_codec.decode(src)? else {
            return Ok(None);
        };

        Ok(Some(serde_json::from_str::<Dec>(&line)?))
    }
}

impl<Dec, Enc, AsRefEnc> Encoder<AsRefEnc> for JsonCodec<Dec, Enc>
where
    Enc: Serialize,
    AsRefEnc: AsRef<Enc>,
{
    type Error = JsonCodecError;

    fn encode(&mut self, item: AsRefEnc, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let item = item.as_ref();
        let item = serde_json::to_string(item)?;

        Ok(self.lines_codec.encode(item, dst)?)
    }
}
