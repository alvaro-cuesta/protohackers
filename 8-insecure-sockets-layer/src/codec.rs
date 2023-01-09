use crate::cipher::Cipher;
use bytes::BytesMut;
use tokio_util::codec::Encoder;

#[derive(Debug)]
pub struct CipherEncoder<Enc, Ciph> {
    inner: Enc,
    cipher: Ciph,
}

impl<Enc, Ciph> CipherEncoder<Enc, Ciph> {
    pub fn new(inner: Enc, cipher: Ciph) -> Self {
        Self { inner, cipher }
    }
}

impl<Enc, Item, Ciph> Encoder<Item> for CipherEncoder<Enc, Ciph>
where
    Enc: Encoder<Item>,
    Ciph: Cipher,
{
    type Error = Enc::Error;

    fn encode(&mut self, item: Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.inner.encode(item, dst)?;

        for byte in dst.iter_mut() {
            *byte = self.cipher.cipher(*byte);
        }

        Ok(())
    }
}
