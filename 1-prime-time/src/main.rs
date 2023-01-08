mod request;
mod response;

use futures::{SinkExt, StreamExt};
use request::Request;
use response::Response;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use utils::{default_listen, framed_json};

async fn handle_client(socket: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
    let mut framed = framed_json::<_, Request, Response>(socket);

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
            Err(_) => {
                let response = Response::error();

                #[cfg(debug_assertions)]
                println!("{addr} <-- {response:?}");

                framed.send(response).await?;

                return Err(anyhow::Error::msg("Malformed response"));
            }
        };
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    default_listen(handle_client).await?;

    Ok(())
}
