use fancy_regex::Regex;
use futures::{SinkExt, StreamExt};
use lazy_static::lazy_static;
use protohackers_utils::{default_tcp_listen, StrictLinesCodec};
use std::{borrow::Cow, net::SocketAddr};
use tokio::{io, net::TcpStream};
use tokio_util::codec::{FramedRead, FramedWrite};

lazy_static! {
    static ref REGEX_BOGUSCOIN: Regex = Regex::new(r"(?<= |^)7[a-zA-Z0-9]{25,34}(?= |$)").unwrap();
}

const TARGET_BOGUSCOIN: &str = "7YWHMfk9JZe0LM0g1ZauHuiSxhI";

fn hack_boguscoin_message(message: &str) -> Cow<'_, str> {
    REGEX_BOGUSCOIN.replace_all(&message, TARGET_BOGUSCOIN)
}

async fn handle_client(client_stream: TcpStream, addr: SocketAddr) -> io::Result<()> {
    let (client_read, client_write) = client_stream.into_split();
    let mut client_read = FramedRead::new(client_read, StrictLinesCodec::new());
    let mut client_write = FramedWrite::new(client_write, StrictLinesCodec::new());

    let server_stream = TcpStream::connect("chat.protohackers.com:16963").await?;
    let (server_read, server_write) = server_stream.into_split();
    let mut server_read = FramedRead::new(server_read, StrictLinesCodec::new());
    let mut server_write = FramedWrite::new(server_write, StrictLinesCodec::new());

    let client_task = tokio::spawn(async move {
        while let Some(message) = client_read.next().await {
            match message {
                Ok(message) => {
                    #[cfg(debug_assertions)]
                    println!("{addr} --> {message}");

                    server_write.send(hack_boguscoin_message(&message)).await?;
                }
                Err(err) => {
                    // TODO: Abort server task
                    return Err(err);
                }
            }
        }

        Ok(())
    });

    let server_task = tokio::spawn(async move {
        while let Some(message) = server_read.next().await {
            match message {
                Ok(message) => {
                    #[cfg(debug_assertions)]
                    println!("{addr} <-- {message}");

                    client_write.send(hack_boguscoin_message(&message)).await?;
                }
                Err(err) => {
                    // TODO: Abort client task
                    return Err(err);
                }
            }
        }

        Ok(())
    });

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    default_tcp_listen(handle_client).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replacement() {
        let result = hack_boguscoin_message("71c0Y37FcpjtZHRLOitTp9NwaY33zM");
        assert_eq!(result, "7YWHMfk9JZe0LM0g1ZauHuiSxhI");

        let result = hack_boguscoin_message("hello 71c0Y37FcpjtZHRLOitTp9NwaY33zM");
        assert_eq!(result, "hello 7YWHMfk9JZe0LM0g1ZauHuiSxhI");

        let result = hack_boguscoin_message("71c0Y37FcpjtZHRLOitTp9NwaY33zM world");

        assert_eq!(result, "7YWHMfk9JZe0LM0g1ZauHuiSxhI world");

        let result = hack_boguscoin_message("hello 71c0Y37FcpjtZHRLOitTp9NwaY33zM world");
        assert_eq!(result, "hello 7YWHMfk9JZe0LM0g1ZauHuiSxhI world");

        let result = hack_boguscoin_message(
            "Please pay the ticket price of 15 Boguscoins to one of these addresses: 75z2OZksnpNqUmGL9M1S4wcvGtDphFytQF 7war1qJPSCwU2a6TQZ07GhKOG2x 7eNq7XxoyVlZBVr30ctqWeN92UfuWaMH");
        assert_eq!(result, "Please pay the ticket price of 15 Boguscoins to one of these addresses: 7YWHMfk9JZe0LM0g1ZauHuiSxhI 7YWHMfk9JZe0LM0g1ZauHuiSxhI 7YWHMfk9JZe0LM0g1ZauHuiSxhI");

        let result = hack_boguscoin_message(
            "This is a product ID, not a Boguscoin: 7l1XLUIvZbaMld8pUX7ncAWhQrkYmtSnmSW-tDAkHyfyWEwfPDnh2WJmakuc4hge8-1234");
        assert_eq!(result, "This is a product ID, not a Boguscoin: 7l1XLUIvZbaMld8pUX7ncAWhQrkYmtSnmSW-tDAkHyfyWEwfPDnh2WJmakuc4hge8-1234");
    }
}
