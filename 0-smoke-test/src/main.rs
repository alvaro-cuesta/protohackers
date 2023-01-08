use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use utils::default_listen;

async fn handle_client(mut socket: TcpStream, _: SocketAddr) -> anyhow::Result<()> {
    let mut buf = vec![0; 1024];

    loop {
        let bytes = socket.read(&mut buf).await?;

        if bytes == 0 {
            break;
        }

        socket.write_all(&buf[0..bytes]).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    default_listen(handle_client).await?;

    Ok(())
}
