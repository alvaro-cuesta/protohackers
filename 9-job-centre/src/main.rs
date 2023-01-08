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
use tokio::{net::TcpStream, sync::Mutex};
use utils::{default_listen, framed_json, JsonCodecError};

// TODO: tokio_serde_json?

async fn handle_client(
    socket: TcpStream,
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<()> {
    let mut framed = framed_json(socket);

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
    // TODO: RwLock?
    let state = Arc::new(Mutex::new(State::default()));

    default_listen(|socket, addr| {
        let state = Arc::clone(&state);
        handle_client(socket, addr, state)
    })
    .await?;

    Ok(())
}
