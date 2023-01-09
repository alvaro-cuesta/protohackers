mod request;
mod response;

use futures::{SinkExt, StreamExt};
use protohackers_utils::{default_tcp_listen, framed_json};
use request::Request;
use response::Response;
use std::net::SocketAddr;
use tokio::net::TcpStream;

async fn handle_client(stream: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
    let mut framed = framed_json::<_, Request, Response>(stream);

    while let Some(item) = framed.next().await {
        #[cfg(debug_assertions)]
        println!("{addr} --> {item:?}");

        match item {
            Ok(Request::IsPrime { number }) => {
                let response = Response::is_prime(number.as_u64().map_or(false, primes::is_prime));

                #[cfg(debug_assertions)]
                println!("{addr} <-- {response:?}");

                framed.send(response).await?;
            }
            Err(err) => {
                let response = Response::error();

                #[cfg(debug_assertions)]
                println!("{addr} <-- {response:?}");

                framed.send(response).await?;

                return Err(err.into());
            }
        };
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    default_tcp_listen(handle_client).await?;

    Ok(())
}
