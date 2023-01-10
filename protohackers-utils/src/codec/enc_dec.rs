use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder};

pub struct EncDecCodec<Enc, Dec> {
    enc: Enc,
    dec: Dec,
}

impl<Enc, Dec> EncDecCodec<Enc, Dec> {
    pub fn new(enc: Enc, dec: Dec) -> Self {
        Self { enc, dec }
    }
}

impl<Enc, Dec, DecItem> Decoder for EncDecCodec<Enc, Dec>
where
    Dec: Decoder<Item = DecItem>,
{
    type Item = DecItem;
    type Error = Dec::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.dec.decode(src)
    }
}

impl<Enc, Dec, EncItem> Encoder<EncItem> for EncDecCodec<Enc, Dec>
where
    Enc: Encoder<EncItem>,
{
    type Error = Enc::Error;

    fn encode(&mut self, item: EncItem, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.enc.encode(item, dst)
    }
}
