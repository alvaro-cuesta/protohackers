use protohackers_utils::default_tcp_listen;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

async fn handle_client(mut stream: TcpStream, _: SocketAddr) -> anyhow::Result<()> {
    let mut buf = vec![0; 1024];

    loop {
        let bytes = stream.read(&mut buf).await?;

        if bytes == 0 {
            break;
        }

        stream.write_all(&buf[0..bytes]).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    default_tcp_listen(handle_client).await?;

    Ok(())
}
