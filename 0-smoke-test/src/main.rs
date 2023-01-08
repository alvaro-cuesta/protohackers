use std::net::SocketAddr;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

async fn process_socket(mut socket: TcpStream, addr: SocketAddr) -> anyhow::Result<()> {
    println!("Got a connection from {addr}");

    socket.set_nodelay(true)?;
    // TODO:
    // socket.set_linger(dur)?;
    // socket.set_ttl(ttl)?;

    let mut buf = vec![0; 1024];

    loop {
        let bytes = socket.read(&mut buf).await?;

        if bytes == 0 {
            break;
        }

        socket.write_all(&buf[0..bytes]).await?;
    }

    println!("Client {addr} disconnected");

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:1337").await?;
    let local_addr = listener.local_addr()?;

    println!("Listening on {local_addr}");

    loop {
        let (socket, addr) = listener.accept().await?;

        // TODO: TcpListenerStream?
        let _: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            process_socket(socket, addr).await?;

            Ok(())
        });
    }
}
