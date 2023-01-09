mod request;
mod response;

use futures::{SinkExt, StreamExt};
use protohackers_utils::{default_listen, FixedSizeCodec, TryFromDecoder, TryIntoEncoder};
use request::Request;
use std::{collections::BTreeMap, net::SocketAddr};
use tokio::net::TcpStream;
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::response::Response;

type Timestamp = i32;

type Price = i32;

// TODO: Arbitrary length integers

async fn handle_client(mut stream: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
    let (read, write) = stream.split();

    let mut read_framed = FramedRead::new(read, FixedSizeCodec::<{ Request::SIZE }>::new())
        .map_decoder(TryFromDecoder::new);
    let mut write_framed =
        FramedWrite::new(write, FixedSizeCodec::<{ i32::BITS as usize / 8 }>::new())
            .map_encoder(TryIntoEncoder::new);

    let mut data: BTreeMap<Timestamp, Price> = BTreeMap::new();

    while let Some(item) = read_framed.next().await {
        #[cfg(debug_assertions)]
        println!("{addr} --> {item:?}");

        let response = match item {
            Ok(Request::Insert { timestamp, price }) => {
                data.insert(timestamp, price);

                None
            }
            Ok(Request::Query { mintime, maxtime }) => {
                if mintime <= maxtime {
                    let (sum, count) = data.range(mintime..=maxtime).try_fold(
                        (0i128, 0i128),
                        |(sum, count), (_, &price)| {
                            Ok::<_, anyhow::Error>((
                                sum.checked_add(price as i128)
                                    .ok_or(anyhow::Error::msg("could not add properly"))?,
                                count + 1,
                            ))
                        },
                    )?;

                    let mean = if count > 0 { sum / count } else { 0 };

                    Some(Response::new(i32::try_from(mean)?))
                } else {
                    Some(Response::new(0))
                }
            }
            Err(err) => return Err(err.into()),
        };

        // TODO: Use response
        if let Some(response) = response {
            #[cfg(debug_assertions)]
            println!("{addr} <-- {response:?}");

            write_framed.send(response).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    default_listen(handle_client).await?;

    Ok(())
}
