use super::{
    message::{LrcpMessage, LrcpMessageError},
    state::LrcpState,
    LrcpSessionHandle, LrcpSessionItem, MAX_PACKET_SIZE,
};
use std::{io, net::SocketAddr, sync::Arc};
use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    sync::Mutex,
};
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Debug)]
pub struct LrcpSocket {
    socket: UdpSocket,
    // TODO: Do I need mutex?
    state: Mutex<LrcpState>,
}

// TODO: Allow initiating a connection from LrcpSocket

impl LrcpSocket {
    pub async fn bind<A: ToSocketAddrs>(
        addr: A,
    ) -> io::Result<(Self, UnboundedReceiverStream<LrcpSessionItem>)> {
        let socket = UdpSocket::bind(addr).await?;
        let (state, recv_session) = LrcpState::new();
        let state = Mutex::new(state);

        Ok((Self { socket, state }, recv_session))
    }

    // TODO: unpub
    pub(super) async fn send_message(
        &self,
        peer: SocketAddr,
        message: &LrcpMessage,
    ) -> io::Result<()> {
        #[cfg(debug_assertions)]
        {
            let handle = LrcpSessionHandle(peer, message.session_id());

            println!("{handle:?} <-- {message:?}");
        }

        let packet = message.to_vec();

        let written = self.socket.send_to(&packet, peer).await?;

        if written != packet.len() {
            return Err(io::Error::new(io::ErrorKind::Other, "Not all bytes sent"));
        }

        #[cfg(debug_assertions)]
        {
            /*
            println!("{peer} <-- {packet:?}");

            if let Ok(reply_packet_str) = std::str::from_utf8(&packet) {
                println!("{peer} <-- {reply_packet_str}");
            }
            */
        }

        Ok(())
    }

    async fn handle_packet(self: &Arc<Self>, packet: &[u8], peer: SocketAddr) -> io::Result<()> {
        // TODO: Problems with backslashes shouldnt crash the whole thing
        let message = receive(peer, packet);

        let Ok(message) = message else  {
            // Problem parsing, silently ignore
            #[cfg(debug_assertions)]
            {
                println!("{peer} -!> Ignoring illegal packet {packet:?}");
            }

            return Ok(())
         };

        let message = match message {
            LrcpMessage::Connect { session } => {
                let must_ack = {
                    let mut state = self.state.lock().await;
                    state
                        .handle_connect(Arc::clone(self), LrcpSessionHandle(peer, session))
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?
                };

                must_ack.then_some(LrcpMessage::Ack { session, length: 0 })
            }
            LrcpMessage::Data {
                session,
                position,
                data,
            } => {
                let ack_pos = {
                    let mut state = self.state.lock().await;
                    state
                        .handle_data(LrcpSessionHandle(peer, session), position, data)
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
                };

                Some(match ack_pos {
                    Some(length) => LrcpMessage::Ack { session, length },
                    None => LrcpMessage::Close { session },
                })
            }
            LrcpMessage::Ack { session, length } => {
                let send_close = {
                    let mut state = self.state.lock().await;
                    state
                        .handle_ack(LrcpSessionHandle(peer, session), length)
                        .await?
                };

                send_close.then_some(LrcpMessage::Close { session })
            }
            LrcpMessage::Close { session } => {
                {
                    let mut state = self.state.lock().await;
                    state
                        .handle_close(LrcpSessionHandle(peer, session))
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
                };

                Some(LrcpMessage::Close { session })
            }
        };

        if let Some(message) = message {
            self.send_message(peer, &message).await?;
        };

        Ok(())
    }

    // TODO: unpub
    pub async fn recv_from(self: Arc<Self>) -> io::Result<()> {
        let mut buf = [0u8; MAX_PACKET_SIZE];
        let (bytes, peer) = self.socket.recv_from(&mut buf).await?;

        let packet = &buf[..bytes];

        let res = self.handle_packet(packet, peer).await;

        #[cfg(debug_assertions)]
        {
            if res.is_err() {
                println!("{peer} <--> {res:?}");
            }
        }

        res
    }

    // TODO: Drop? TcpStream does not have `close`
    async fn close(self) {
        unimplemented!()
    }
}

// TODO: Drop? TcpStream does not have `close`

fn receive(peer: SocketAddr, packet: &[u8]) -> Result<LrcpMessage, LrcpMessageError> {
    #[cfg(debug_assertions)]
    {
        /*
        println!("{peer} --> {packet:?}");

        if let Ok(packet_str) = std::str::from_utf8(packet) {
            println!("{peer} --> {packet_str}");
        }
        */
    }

    let message = LrcpMessage::from(packet)?;

    #[cfg(debug_assertions)]
    {
        let handle = LrcpSessionHandle(peer, message.session_id());
        println!("{handle:?} --> {message:?}");
    }

    Ok(message)
}
