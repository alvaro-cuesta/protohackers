use futures::Future;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_stream::{wrappers::TcpListenerStream, StreamExt};

#[derive(Debug, Error)]
pub enum ListenError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub const DEFAULT_IPV4_ADDR: &str = "0.0.0.0:1337";
pub const DEFAULT_IPV6_ADDR: &str = "[::]:1337";

pub async fn default_tcp_listen<F, Fut, E>(handle_client: F) -> std::io::Result<()>
where
    F: Fn(TcpStream, SocketAddr) -> Fut,
    Fut: Future<Output = Result<(), E>> + Send + 'static,
    E: Send + 'static,
{
    let listener_v4 = TcpListener::bind(DEFAULT_IPV4_ADDR).await?;
    let listener_v6 = TcpListener::bind(DEFAULT_IPV6_ADDR).await?;

    let local_addr_v4 = listener_v4.local_addr()?;
    let local_addr_v6 = listener_v6.local_addr()?;

    println!("Listening on {local_addr_v4} (v4) {local_addr_v6} (v6)");

    let mut incoming =
        TcpListenerStream::new(listener_v4).merge(TcpListenerStream::new(listener_v6));

    while let Some(stream) = incoming.next().await {
        let stream = stream?;

        let addr = stream.peer_addr()?;

        println!("Got a connection from {addr}");

        stream.set_nodelay(true)?;
        // TODO: Maybe use:
        // stream.set_linger(dur)?;
        // stream.set_ttl(ttl)?;

        let client_future = handle_client(stream, addr);

        let _: JoinHandle<Result<(), E>> = tokio::spawn(async move {
            client_future.await?;

            println!("Client {addr} disconnected");

            Ok(())
        });
    }

    Ok(())
}
