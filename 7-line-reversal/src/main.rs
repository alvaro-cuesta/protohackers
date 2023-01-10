#![feature(array_windows)]

mod lrcp;

use futures::SinkExt;
use lrcp::{LrcpSessionHandle, LrcpSocket};
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    task::JoinHandle,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

type SessionId = u32;

async fn handle_client(
    session: LrcpSessionHandle,
    read: impl AsyncRead + Unpin,
    write: impl AsyncWrite + Unpin,
) -> Result<(), anyhow::Error> {
    let mut read_framed = FramedRead::new(read, LinesCodec::new());
    let mut write_framed = FramedWrite::new(write, LinesCodec::new());

    while let Some(item) = read_framed.next().await {
        #[cfg(debug_assertions)]
        println!("{session:?} ==> {item:?}");

        let item = item?;

        let reverse = item.chars().rev().collect::<String>().into_bytes();
        let reverse_str = std::str::from_utf8(&reverse)?;

        println!("{session:?} <== {reverse_str:?}");

        write_framed.send(reverse_str).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO: Make listener?
    let (socket, mut recv_session) = LrcpSocket::bind("0.0.0.0:1337").await?;
    let socket = Arc::new(socket);

    // TODO  Handle errors, timeouts, etc. not only data
    let _: JoinHandle<Result<(), anyhow::Error>> = {
        tokio::spawn(async move {
            while let Some((session, read, writer)) = recv_session.next().await {
                let _: JoinHandle<Result<(), anyhow::Error>> = tokio::spawn(async move {
                    handle_client(session, read, writer).await?;

                    Ok(())
                });
            }

            Ok(())
        })
    };

    // TODO: Move this to actor pattern
    let _: Result<(), anyhow::Error> = tokio::spawn(async move {
        loop {
            let socket = Arc::clone(&socket);
            LrcpSocket::recv_from(socket).await?;
        }
    })
    .await?;

    Ok(())
}
