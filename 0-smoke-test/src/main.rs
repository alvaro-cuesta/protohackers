use futures::StreamExt;
use protohackers_utils::default_tcp_listen;
use std::net::SocketAddr;
use tokio::{io, net::TcpStream};
use tokio_util::codec::{BytesCodec, Framed};

async fn handle_client(stream: TcpStream, _: SocketAddr) -> io::Result<()> {
    let (write, read) = Framed::new(stream, BytesCodec::new()).split();
    read.forward(write).await
}

#[tokio::main]
async fn main() -> io::Result<()> {
    default_tcp_listen(handle_client).await
}
