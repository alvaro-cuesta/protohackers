use protohackers_utils::DEFAULT_IPV4_ADDR;
use std::{
    collections::HashMap,
    io::{self, Write},
    net::SocketAddr,
};
use tokio::net::UdpSocket;

const MAX_MESSAGE_SIZE: usize = 1000;

const VERSION_KEY: &[u8] = b"version";

const VERSION_VALUE: &[u8] =
    concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION")).as_bytes();

struct Request {
    from: SocketAddr,
    key: Vec<u8>,
    value: Option<Vec<u8>>,
}

async fn read_request_from_socket(
    socket: &UdpSocket,
    buf: &mut [u8; MAX_MESSAGE_SIZE],
) -> io::Result<Option<Request>> {
    let (read, addr) = socket.recv_from(buf).await?;
    let message = buf[..read].to_owned();

    #[cfg(debug_assertions)]
    {
        println!("{addr} --> {message:?}");

        if let Ok(message_str) = std::str::from_utf8(&message) {
            println!("{addr} --> {message_str}");
        }
    }

    let mut iter = message.splitn(2, |x| *x == b'=');
    let Some(key) = iter.next() else {
        return Ok(None);
    };
    let value = iter.next();
    assert!(iter.next().is_none());

    Ok(Some(Request {
        from: addr,
        key: key.to_owned(),
        value: value.map(ToOwned::to_owned),
    }))
}

async fn write_request_to_socket(
    socket: &UdpSocket,
    buf: &mut [u8; MAX_MESSAGE_SIZE],
    addr: SocketAddr,
    key: &[u8],
    value: &[u8],
) -> io::Result<()> {
    let total_len = key.len() + 1 + value.len();

    if total_len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "tried to write too many bytes",
        ));
    }

    let mut write_buf = buf.as_mut_slice();

    if let Err(_) = write_buf.write_all(&key) {
        return Ok(());
    };
    if let Err(_) = write_buf.write_all(b"=") {
        return Ok(());
    };
    if let Err(_) = write_buf.write_all(&value) {
        return Ok(());
    };

    let message = &buf[..total_len];

    #[cfg(debug_assertions)]
    {
        println!("{addr:?} <-- {message:?}");

        if let Ok(message_str) = std::str::from_utf8(&message) {
            println!("{addr} <-- {message_str}");
        }
    }

    socket.send_to(message, addr).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let socket = UdpSocket::bind(DEFAULT_IPV4_ADDR).await?;

    let mut read_buf = [0u8; MAX_MESSAGE_SIZE];
    let mut write_buf = [0u8; MAX_MESSAGE_SIZE];
    let mut state = HashMap::new();

    loop {
        let Some(Request { from, key, value }) = read_request_from_socket(&socket, &mut read_buf).await? else {
            continue;
        };

        match value {
            Some(_) if key == VERSION_KEY => {
                continue;
            }
            Some(value) => {
                state.insert(key, value);
            }
            None if key == VERSION_KEY => {
                write_request_to_socket(&socket, &mut write_buf, from, &key, VERSION_VALUE).await?;
            }
            None => {
                let Some(value) = state.get(&key) else { continue };
                write_request_to_socket(&socket, &mut write_buf, from, &key, value).await?;
            }
        }
    }
}
