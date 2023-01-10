use super::{
    session_write::{LrcpSessionBuffer, LrcpSessionWrite},
    LrcpSocket,
};
use crate::{
    lrcp::{LrcpSessionError, LrcpSessionItem, LrcpSessionReadItem},
    LrcpSessionHandle,
};
use bytes::Bytes;
use std::{
    collections::{hash_map::Entry, HashMap},
    io,
    sync::Arc,
};
use tokio::sync::{
    mpsc::{self, error::SendError},
    Mutex,
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::io::StreamReader;

#[derive(Debug)]
pub struct LrcpState {
    // TODO: Make AsyncRead, AsyncWrite
    session_send: mpsc::UnboundedSender<LrcpSessionItem>,
    sessions: HashMap<LrcpSessionHandle, LrcpSession>,
}

// TODO: Avoid `as`

impl LrcpState {
    pub fn new() -> (Self, UnboundedReceiverStream<LrcpSessionItem>) {
        let (session_send, session_recv) = mpsc::unbounded_channel();

        let session_stream = UnboundedReceiverStream::new(session_recv);

        let state = Self {
            session_send,
            sessions: Default::default(),
        };

        (state, session_stream)
    }

    pub fn handle_connect(
        &mut self,
        socket: Arc<LrcpSocket>,
        session: LrcpSessionHandle,
    ) -> Result<bool, SendError<LrcpSessionItem>> {
        match self.sessions.entry(session) {
            Entry::Occupied(o) => match o.get().state {
                LrcpSessionState::Opening => Ok(true),
                _ => Ok(false),
            },
            Entry::Vacant(v) => {
                let (data_send, data_recv) = mpsc::unbounded_channel::<LrcpSessionReadItem>();
                let data_recv = UnboundedReceiverStream::new(data_recv);
                let data_recv = StreamReader::new(data_recv);

                let (lrcp_session, writer) = LrcpSession::new(socket, session, data_send);

                v.insert(lrcp_session);

                self.session_send.send((session, data_recv, writer))?;

                Ok(true)
            }
        }
    }

    pub fn handle_data(
        &mut self,
        session: LrcpSessionHandle,
        position: u32,
        mut data: Vec<u8>,
    ) -> Result<Option<u32>, SendError<LrcpSessionReadItem>> {
        let Entry::Occupied(mut session_state) = self.sessions.entry(session) else {
            return Ok(None)
        };
        let session_state = session_state.get_mut();

        if session_state.state == LrcpSessionState::Opening {
            session_state.state = LrcpSessionState::Running;
        }

        if position > session_state.received_len {
            // Packet is in the future, we might be receiving out-of-order packet
            // Ignore but retransmit ack to re-request in order
            return Ok(Some(session_state.received_len));
        }

        let drop_len = session_state.received_len - position;

        if drop_len < data.len() as u32 {
            // Drop overlapping data
            data.drain(..drop_len as usize);

            // Update position and yield bytes
            session_state.received_len += data.len() as u32;
            session_state.data_send.send(Ok(Bytes::from(data)))?;
        }

        // Even if we didn't actually read anything, transmit ack so we re-request what we need
        Ok(Some(session_state.received_len))
    }

    /// Returns bool if it should close the connection
    pub async fn handle_ack(
        &mut self,
        session: LrcpSessionHandle,
        length: u32,
    ) -> io::Result<bool> {
        match self.sessions.entry(session) {
            Entry::Occupied(mut o) => {
                let misbehaved = o.get_mut().buffer.lock().await.handle_ack(length).await?;

                if misbehaved {
                    o.remove()
                        .data_send
                        // TODO: Shoudl probably behave like EOF not error
                        .send(Err(LrcpSessionError::PeerMisbehaved))
                        .map_err(|_| {
                            io::Error::new(io::ErrorKind::Other, "Nobody listening to data")
                        })?;
                }

                Ok(misbehaved)
            }
            Entry::Vacant(_) => Ok(true),
        }
    }

    pub fn handle_close(
        &mut self,
        session: LrcpSessionHandle,
    ) -> Result<(), SendError<LrcpSessionReadItem>> {
        if let Entry::Occupied(o) = self.sessions.entry(session) {
            o.remove()
                .data_send
                // TODO: Shoudl probably behave like EOF not error
                .send(Err(LrcpSessionError::PeerClosed))?;
        }

        Ok(())
    }

    // TODO: Timeout remove sessions
}

#[derive(Debug)]
struct LrcpSession {
    received_len: u32,
    data_send: mpsc::UnboundedSender<LrcpSessionReadItem>,
    buffer: Arc<Mutex<LrcpSessionBuffer>>,
    state: LrcpSessionState,
}

impl LrcpSession {
    fn new(
        socket: Arc<LrcpSocket>,
        session: LrcpSessionHandle,
        data_send: mpsc::UnboundedSender<LrcpSessionReadItem>,
    ) -> (Self, LrcpSessionWrite) {
        let (buffer, writer) = LrcpSessionBuffer::new(socket, session, None);

        let session = Self {
            received_len: 0,
            data_send,
            buffer,
            state: LrcpSessionState::Opening,
        };

        (session, writer)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LrcpSessionState {
    Opening,
    Running,
}
