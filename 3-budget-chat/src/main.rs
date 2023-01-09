mod state;

use futures::{SinkExt, StreamExt};
use protohackers_utils::default_tcp_listen;
use state::{Event, State};
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpStream, select, sync::RwLock};
use tokio_util::codec::{Framed, LinesCodec};

type ClientName = String;

type Message = String;

async fn handle_joined(
    mut framed: Framed<TcpStream, LinesCodec>,
    addr: SocketAddr,
    name: String,
    state: Arc<RwLock<State>>,
) -> anyhow::Result<()> {
    let mut receive = {
        let mut state = state.write().await;
        state.add_client(name.clone())?
    };

    loop {
        select! {
            item = framed.next() => {
                #[cfg(debug_assertions)]
                println!("{addr} --> {item:?}");

                let message = item.ok_or(anyhow::Error::msg("Got EOF"))??;
                let message = message;

                {
                    let mut state = state.write().await;
                    state.broadcast_message(name.clone(), message);
                }
            }
            event = receive.recv() => {
                let event = event.ok_or(anyhow::Error::msg("Somohow all senders dropped?"))?;

                match event {
                    Event::NewClient(name) => {
                        framed
                            .send(format!("* {name} has entered the room"))
                            .await?;
                    },
                    Event::Message(from, message) => {
                        framed
                            .send(format!("[{from}] {message}"))
                            .await?;
                    },
                    Event::Disconnect(name) => {
                        framed
                            .send(format!("* {name} has left the room"))
                            .await?;
                    },
                }
            }
        }
    }
}

async fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<RwLock<State>>,
) -> anyhow::Result<()> {
    let mut framed = Framed::new(stream, LinesCodec::new());

    framed
        .send("Welcome to budgetchat! What shall I call you?")
        .await?;

    let name = framed.next().await.ok_or(anyhow::Error::msg("Got EOF"))??;

    if name.is_empty() {
        framed.send(format!("Your name cannot be empty")).await?;

        return Err(anyhow::Error::msg("Illegal empty name"));
    } else if !name.chars().all(|char| {
        ('a'..='z').contains(&char) || ('A'..='Z').contains(&char) || ('0'..='9').contains(&char)
    }) {
        framed
            .send(format!(
                "Your name can only have alphanumeric characters (uppercase, lowercase, digits)"
            ))
            .await?;

        return Err(anyhow::Error::msg("Illegal characters in name"));
    }

    let online = {
        let state = state.read().await;
        state.get_present_names().join(", ")
    };

    framed
        .send(format!("* The room contains: {online}"))
        .await?;

    let result = handle_joined(framed, addr, name.clone(), state.clone()).await;

    {
        let mut state = state.write().await;
        state.disconnect_client(name);
    }

    result
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO: RwLock?
    let state = Arc::new(RwLock::new(State::default()));

    default_tcp_listen(|stream, addr| {
        let state = Arc::clone(&state);
        handle_client(stream, addr, state)
    })
    .await?;

    Ok(())
}
