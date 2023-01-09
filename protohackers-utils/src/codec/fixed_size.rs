use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Default)]
pub struct FixedSizeCodec<const N: usize>;

impl<const N: usize> FixedSizeCodec<N> {
    pub fn new() -> Self {
        Self
    }
}

impl<const N: usize> Decoder for FixedSizeCodec<N> {
    type Item = [u8; N];
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < N {
            return Ok(None);
        }

        let data: Self::Item = src[..N]
            .try_into()
            // Size checked above
            .unwrap();

        src.advance(N);

        Ok(Some(data))
    }
}

impl<const N: usize> Encoder<[u8; N]> for FixedSizeCodec<N> {
    type Error = std::io::Error;

    fn encode(&mut self, item: [u8; N], dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(N);

        dst.extend_from_slice(&item);

        Ok(())
    }
}
