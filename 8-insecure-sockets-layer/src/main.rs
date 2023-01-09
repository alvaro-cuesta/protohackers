mod cipher;
mod codec;

use bytes::Bytes;
use cipher::{Cipher, ComposedCipher};
use codec::CipherEncoder;
use futures::{SinkExt, StreamExt};
use protohackers_utils::default_listen;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpStream,
};
use tokio_util::{
    codec::{FramedRead, FramedWrite, LinesCodec},
    io::{ReaderStream, StreamReader},
};

async fn handle_client(mut stream: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
    let (read, write) = stream.split();

    let (read, cipher_spec) = {
        let mut read = BufReader::new(read);
        let mut cipher_spec = Vec::new();

        read.read_until(0x00, &mut cipher_spec).await?;

        (read.into_inner(), cipher_spec)
    };

    let ciphers = ComposedCipher::from_spec_slice(&cipher_spec[..cipher_spec.len() - 1])
        .ok_or(anyhow::Error::msg("Invalid cipher spec"))?;

    println!("{addr} wants ciphers: {ciphers:?}");

    if ciphers.check_is_noop() {
        return Err(anyhow::Error::msg("Tried to use a no-op cipher"));
    }

    let mut read_cipher = ciphers.clone();

    let read = ReaderStream::new(read).map(|item| {
        item.map(|bytes| {
            bytes
                .iter()
                .map(|byte| read_cipher.decipher(*byte))
                .collect::<Bytes>()
        })
    });

    let read = StreamReader::new(read);

    let mut read_framed = FramedRead::new(read, LinesCodec::new());

    let mut write_framed = FramedWrite::new(write, CipherEncoder::new(LinesCodec::new(), ciphers));

    while let Some(item) = read_framed.next().await {
        #[cfg(debug_assertions)]
        println!("{addr} --> {item:?}");

        let item = item?;

        let parsed_items = item
            .split(",")
            .map(|item| {
                let (num, _) = item.split_once("x")?;
                let num = num.parse::<u32>().ok()?;

                Some((num, item))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(anyhow::Error::msg("Received some wrong item"))?;

        let (_, max_item) = parsed_items
            .iter()
            .max_by_key(|(num, _)| num)
            .ok_or(anyhow::Error::msg("Received empty list"))?;

        write_framed.send(max_item).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    default_listen(handle_client).await?;

    Ok(())
}
