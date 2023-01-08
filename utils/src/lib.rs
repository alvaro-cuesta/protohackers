mod json_codec;
mod listen;

pub use json_codec::*;
pub use listen::*;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

pub fn framed_json<RW, Req, Res>(rw: RW) -> Framed<RW, JsonCodec<Req, Res>>
where
    RW: AsyncRead + AsyncWrite,
{
    Framed::new(rw, JsonCodec::<Req, Res>::new())
}
