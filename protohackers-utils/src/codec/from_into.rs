use std::marker::PhantomData;
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Error)]
pub enum TryFromDecoderError<DecErr, TryFromErr> {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("decoder error: {0}")]
    Decoder(DecErr),
    #[error("try_from error: {0}")]
    TryFrom(TryFromErr),
}

#[derive(Debug, Default)]
pub struct TryFromDecoder<Dec, Item> {
    inner: Dec,
    into: PhantomData<Item>,
}

impl<Dec, Item> TryFromDecoder<Dec, Item> {
    pub fn new(inner: Dec) -> Self {
        Self {
            inner,
            into: PhantomData::default(),
        }
    }
}

impl<Dec, Item, TryFromErr> Decoder for TryFromDecoder<Dec, Item>
where
    Dec: Decoder,
    Item: TryFrom<Dec::Item, Error = TryFromErr>,
{
    type Item = Item;
    type Error = TryFromDecoderError<Dec::Error, TryFromErr>;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let Some(item_from) = self.inner.decode(src).map_err(TryFromDecoderError::Decoder)? else {
            return Ok(None);
        };

        Ok(Some(
            Item::try_from(item_from).map_err(TryFromDecoderError::TryFrom)?,
        ))
    }
}

#[derive(Debug, Error)]
pub enum TryIntoEncoderError<EncErr, TryIntoErr> {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encoder error: {0}")]
    Encoder(EncErr),
    #[error("try_into error: {0}")]
    TryInto(TryIntoErr),
}

#[derive(Debug, Default)]
pub struct TryIntoEncoder<Enc, EncItem, Item> {
    inner: Enc,
    enc_item: PhantomData<EncItem>,
    item: PhantomData<Item>,
}

impl<Enc, EncItem, Item> TryIntoEncoder<Enc, EncItem, Item> {
    pub fn new(inner: Enc) -> Self {
        Self {
            inner,
            enc_item: PhantomData::default(),
            item: PhantomData::default(),
        }
    }
}

impl<Enc, EncItem, Item, TryIntoErr> Encoder<Item> for TryIntoEncoder<Enc, EncItem, Item>
where
    EncItem: TryFrom<Item, Error = TryIntoErr>,
    Enc: Encoder<EncItem>,
{
    type Error = TryIntoEncoderError<Enc::Error, TryIntoErr>;

    fn encode(&mut self, item: Item, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        let item = EncItem::try_from(item).map_err(TryIntoEncoderError::TryInto)?;

        Ok(self
            .inner
            .encode(item, dst)
            .map_err(TryIntoEncoderError::Encoder)?)
    }
}
