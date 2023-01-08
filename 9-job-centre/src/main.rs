// TODO: Zero copy serde?
// TODO: Is it still leakingo on hard closes?

mod job;
mod request;
mod response;
mod state;

use crate::{
    request::Request,
    response::Response,
    state::{State, WaitResponse},
};
use futures::{SinkExt, StreamExt};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
    task::JoinHandle,
};
use tokio_util::codec::Framed;
use utils::{JsonCodec, JsonCodecError};

// TODO: tokio_serde_json?

async fn process_socket(
    socket: TcpStream,
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<()> {
    println!("Got a connection from {addr}");

    socket.set_nodelay(true)?;
    // TODO:
    // socket.set_linger(dur)?;
    // socket.set_ttl(ttl)?;

    let mut framed = Framed::new(socket, JsonCodec::<Request, Response>::new());

    while let Some(item) = framed.next().await {
        #[cfg(debug_assertions)]
        println!("{addr} --> {item:?}");

        let response = match item {
            Ok(Request::Put { queue, job, pri }) => {
                let id = {
                    let mut state = state.lock().await;
                    state.add_job(queue, job, pri)
                };

                Response::ok_put(id)
            }
            Ok(Request::Get { queues, wait }) => {
                if !wait {
                    let get_job_response = {
                        let mut state = state.lock().await;
                        state.get_job(addr, queues)
                    };

                    match get_job_response {
                        Some(full_job) => Response::ok_get(full_job),
                        None => Response::NoJob,
                    }
                } else {
                    let wait_response = {
                        let mut state = state.lock().await;
                        state.wait_job(addr, queues)
                    };

                    let full_job = match wait_response {
                        WaitResponse::Job(full_job) => full_job,
                        WaitResponse::Wait(receiver) => receiver.await?,
                    };

                    Response::ok_get(full_job)
                }
            }
            Ok(Request::Delete { id }) => {
                let delete_response = {
                    let mut state = state.lock().await;
                    state.delete_job(id)
                };

                if delete_response {
                    Response::ok_delete()
                } else {
                    Response::no_job()
                }
            }
            Ok(Request::Abort { id }) => {
                let abort_response = {
                    let mut state = state.lock().await;
                    state.abort_job(addr, id)
                };

                if abort_response {
                    Response::ok_abort()
                } else {
                    Response::no_job()
                }
            }
            #[cfg(debug_assertions)]
            Ok(Request::Debug) => {
                {
                    let state = state.lock().await;
                    println!("{state:#?}");
                }

                Response::ok_debug()
            }
            Err(JsonCodecError::SerdeJson(_)) => Response::error("Invalid request".to_string()),
            Err(err @ JsonCodecError::Io(_) | err @ JsonCodecError::LinesCodec(_)) => {
                return Err(err.into())
            }
        };

        #[cfg(debug_assertions)]
        println!("{addr} <-- {response:?}");

        framed.send(&response).await?;
    }

    println!("Client {addr} disconnected");

    {
        let mut state = state.lock().await;
        state.abort_client_jobs(addr);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:1337").await?;
    let local_addr = listener.local_addr()?;

    println!("Listening on {local_addr}");

    // TODO: RwLock?
    let state = Arc::new(Mutex::new(State::default()));

    loop {
        let (socket, addr) = listener.accept().await?;

        let state = Arc::clone(&state);

        // TODO: TcpListenerStream?
        let _: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            process_socket(socket, addr, state).await?;

            Ok(())
        });
    }
}
