mod codec;
mod heartbeat;
mod message;
mod parse;
mod state;

use codec::{MessageToClientEncoder, MessageToServerDecoder};
use futures::{SinkExt, Stream, StreamExt};
use heartbeat::Heartbeat;
use message::{MessageToClient, MessageToServer};
use protohackers_utils::default_tcp_listen;
use state::State;
use std::{io, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{mpsc, Mutex},
};
use tokio_util::codec::{FramedRead, FramedWrite};

type Road = u16;

type Mile = u16;

type Timestamp = u32;

type Speed = u16;

type Plate = String;

type HeartbeatInterval = Duration;

async fn handle_camera(
    addr: SocketAddr,
    mut read: impl Stream<Item = Result<MessageToServer, io::Error>> + Unpin,
    write: mpsc::Sender<MessageToClient>,
    mut heartbeat: Heartbeat,
    road: Road,
    mile: Mile,
    limit: Speed,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<()> {
    while let Some(message) = read.next().await {
        #[cfg(debug_assertions)]
        {
            println!("{addr:?} --> [C] {message:?} (road {road}, mile {mile}, limit {limit})");
        }

        match message {
            Ok(MessageToServer::Plate { plate, timestamp }) => {
                let mut state = state.lock().await;
                state
                    .report_plate(road, mile, limit, plate, timestamp)
                    .await;
            }
            Ok(MessageToServer::WantHeartbeat { interval }) => {
                if let Err(err) = heartbeat.start(write.clone(), interval) {
                    return handle_forward_err(write, err, "invalid message").await;
                }
            }
            Ok(message) => {
                return handle_dynamic_err(
                    write,
                    format!("expected camera message, got {message:?}"),
                )
                .await
            }
            Err(err) => return handle_forward_err(write, err, "invalid message").await,
        }
    }

    Ok(())
}

async fn handle_dispatcher_loop(
    addr: SocketAddr,
    mut read: impl Stream<Item = Result<MessageToServer, io::Error>> + Unpin,
    write: mpsc::Sender<MessageToClient>,
    mut heartbeat: Heartbeat,
) -> anyhow::Result<()> {
    while let Some(message) = read.next().await {
        #[cfg(debug_assertions)]
        {
            println!("{addr:?} --> [D] {message:?}");
        }

        match message {
            Ok(MessageToServer::WantHeartbeat { interval }) => {
                if let Err(err) = heartbeat.start(write.clone(), interval) {
                    return handle_forward_err(write, err, "invalid message").await;
                }
            }
            Ok(message) => {
                return handle_dynamic_err(
                    write,
                    format!("expected dispatcher message, got {message:?}"),
                )
                .await
            }
            Err(err) => return handle_forward_err(write, err, "invalid message").await,
        }
    }

    Ok(())
}

async fn handle_dispatcher(
    addr: SocketAddr,
    read: impl Stream<Item = Result<MessageToServer, io::Error>> + Unpin,
    write: mpsc::Sender<MessageToClient>,
    heartbeat: Heartbeat,
    roads: Vec<Road>,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<()> {
    {
        let mut state = state.lock().await;
        state.insert_dispatcher(&roads, addr, write.clone()).await;
    }

    let res = handle_dispatcher_loop(addr, read, write, heartbeat).await;

    {
        let mut state = state.lock().await;
        state.remove_dispatcher(&roads, addr);
    }

    res
}

async fn handle_err(write: mpsc::Sender<MessageToClient>, message: &str) -> anyhow::Result<()> {
    write.send(MessageToClient::error(message)).await?;
    Ok(())
}

async fn handle_dynamic_err<T>(
    write: mpsc::Sender<MessageToClient>,
    message: String,
) -> anyhow::Result<T> {
    handle_err(write, &message).await?;
    Err(anyhow::Error::msg(message))
}

async fn handle_forward_err<T, E: Into<anyhow::Error>>(
    write: mpsc::Sender<MessageToClient>,
    err: E,
    message: &'static str,
) -> anyhow::Result<T> {
    handle_err(write, message).await?;
    Err(err.into())
}

async fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<()> {
    let (read, write) = stream.into_split();
    let mut read = FramedRead::new(read, MessageToServerDecoder);
    let mut write = FramedWrite::new(write, MessageToClientEncoder);
    let (write_send, mut write_recv) = mpsc::channel::<MessageToClient>(1);

    tokio::spawn(async move {
        while let Some(message) = write_recv.recv().await {
            let is_error = message.is_error();

            #[cfg(debug_assertions)]
            {
                println!("{addr:?} <-- {message:?}");
            }

            write.send(message).await?;

            if is_error {
                write.into_inner().shutdown().await?;
                break;
            }
        }

        Ok::<_, io::Error>(())
    });

    let mut heartbeat = Heartbeat::new();

    while let Some(message) = read.next().await {
        #[cfg(debug_assertions)]
        {
            println!("{addr:?} --> [?] {message:?}");
        }

        match message {
            Ok(MessageToServer::IAmCamera { road, mile, limit }) => {
                return handle_camera(addr, read, write_send, heartbeat, road, mile, limit, state)
                    .await
            }
            Ok(MessageToServer::IAmDispatcher { roads }) => {
                return handle_dispatcher(addr, read, write_send, heartbeat, roads, state).await
            }
            Ok(MessageToServer::WantHeartbeat { interval }) => {
                if let Err(err) = heartbeat.start(write_send.clone(), interval) {
                    return handle_forward_err(write_send, err, "invalid message").await;
                }
            }
            Ok(message) => {
                return handle_dynamic_err(
                    write_send,
                    format!("expected initial message, got {message:?}"),
                )
                .await
            }
            Err(err) => return handle_forward_err(write_send, err, "invalid message").await,
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(State::new()));

    default_tcp_listen(|stream, addr| {
        let state = Arc::clone(&state);

        tokio::spawn(async move {
            let res = handle_client(stream, addr, state).await;

            if let Err(err) = res {
                println!("[ERR] {addr:?} {err:?}");
            }
        })
    })
    .await?;

    Ok(())
}
