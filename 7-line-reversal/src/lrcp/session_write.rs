use super::{message::LrcpMessage, LrcpSessionHandle, LrcpSocket, MAX_DATA_SIZE};
use futures::{future::BoxFuture, ready, Future};
use std::{
    cmp::Ordering,
    fmt::Debug,
    io,
    pin::Pin,
    sync::{
        atomic::{self, AtomicU32},
        Arc,
    },
    task::{Context, Poll, Waker},
    time::Duration,
};
use tokio::{
    io::AsyncWrite,
    sync::{watch, Mutex},
    time::MissedTickBehavior,
};

const RETRANSMISSION_TIMEOUT: Duration = Duration::from_secs(3);

const SESSION_EXPIRY_TIMEOUT: Duration = Duration::from_secs(60);

// TODO: Atomic care about ordering?

// TODO: Make an enum of closed, errored, etc.?
#[derive(Debug)]
pub(super) struct LrcpSessionBuffer {
    buffer: Vec<u8>,
    max_buffer_size: Option<usize>,
    // TODO: This is scary, possible mem leak due to circular Arc with socket owning the buffer
    socket: Arc<LrcpSocket>,
    session: LrcpSessionHandle,
    position: Arc<AtomicU32>,
    is_empty_send: watch::Sender<bool>,
    // TODO: Split in capacity/flushing finish?
    // TODO: Unset when not used?
    last_waker_recv: watch::Receiver<Option<Waker>>,
}

impl LrcpSessionBuffer {
    pub fn new(
        socket: Arc<LrcpSocket>,
        session: LrcpSessionHandle,
        max_buffer_size: Option<usize>,
    ) -> (Arc<Mutex<Self>>, LrcpSessionWrite) {
        let (is_empty_send, is_empty_recv) = watch::channel(true);
        let (last_waker_send, last_waker_recv) = watch::channel(None);

        let buffer = Self {
            buffer: Vec::default(),
            max_buffer_size: max_buffer_size,
            socket: Arc::clone(&socket),
            session,
            position: Arc::new(AtomicU32::new(0)),
            is_empty_send,
            last_waker_recv,
        };

        let buffer = Arc::new(Mutex::new(buffer));

        let writer = LrcpSessionWrite::new(
            Arc::clone(&buffer),
            last_waker_send,
            is_empty_recv,
            Arc::clone(&socket),
            session,
        );

        (buffer, writer)
    }

    async fn transmit_buf(&self, buf: &[u8], position: u32) -> io::Result<()> {
        // TODO: This is flawed? Shouldn't start retransmitting immediately, only in sequence?
        for (i, chunk) in buf.chunks(MAX_DATA_SIZE).enumerate() {
            self.transmit_buf_chunk(chunk, (i * MAX_DATA_SIZE) as u32 + position)
                .await?;
        }

        Ok(())
    }

    async fn transmit_buf_chunk(&self, buf: &[u8], position: u32) -> io::Result<()> {
        assert!(buf.len() <= MAX_DATA_SIZE);

        // TODO: Split in chunks
        let LrcpSessionHandle(peer, session) = self.session;

        // TODO: Abstract this a bit, here I shouldn't know about message
        let message = LrcpMessage::Data {
            session,
            position,
            data: buf.to_owned(),
        };

        self.socket.send_message(peer, &message).await?;
        let mut retransmit_interval = tokio::time::interval(RETRANSMISSION_TIMEOUT);
        retransmit_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        retransmit_interval.tick().await;

        // Spawn the retransmission task
        tokio::spawn({
            let expected_ack = position + buf.len() as u32;
            let last_ack = Arc::clone(&self.position);
            let socket = Arc::clone(&self.socket);

            async move {
                let retransmit_timeout = tokio::time::timeout(SESSION_EXPIRY_TIMEOUT, async move {
                    loop {
                        retransmit_interval.tick().await;

                        if last_ack.load(atomic::Ordering::Relaxed) >= expected_ack {
                            return Ok::<_, io::Error>(());
                        }

                        let handle = LrcpSessionHandle(peer, message.session_id());
                        println!("{handle:?} <!- [RETRANSMIT] {message:?}",);

                        socket.send_message(peer, &message).await?;
                    }
                });

                match retransmit_timeout.await {
                    Ok(_) => {}
                    Err(elapsed) => {
                        // TODO: Error out
                        unimplemented!()
                    }
                }
            }
        });

        Ok(())
    }

    async fn write(&mut self, buf: &[u8]) -> io::Result<Option<usize>> {
        let buf = match self.max_buffer_size {
            Some(max_buffer_size) => {
                let capacity = max_buffer_size - self.buffer.len();

                if capacity == 0 {
                    return Ok(None);
                }

                &buf[..capacity]
            }
            None => buf,
        };

        self.transmit_buf(
            buf,
            self.position.load(atomic::Ordering::Relaxed) + self.buffer.len() as u32,
        )
        .await?;
        self.buffer.extend_from_slice(buf);

        Ok(Some(buf.len()))
    }

    /// Returns `true` if ack was misbehaved
    pub(super) async fn handle_ack(&mut self, ack_position: u32) -> io::Result<bool> {
        let was_empty = self.buffer.is_empty();

        if ack_position <= self.position.load(atomic::Ordering::Relaxed) {
            // Possibly a delayed ack, ignore
            return Ok(false);
        }

        let sent_position =
            self.position.load(atomic::Ordering::Relaxed) + self.buffer.len() as u32;

        let misbehaved = match ack_position.cmp(&sent_position) {
            Ordering::Greater => {
                // Remote peer is misbehaving -- Signal that we should cut the cord
                // TODO: Close self? Or is it done automatically by dropping session elsewhere?
                true
            }
            Ordering::Less => {
                // Ack still wants data, retransmit it
                let ack_index = ack_position - self.position.load(atomic::Ordering::Relaxed);

                self.buffer.drain(..ack_index as usize);
                self.position.store(ack_position, atomic::Ordering::Relaxed);

                self.transmit_buf(&self.buffer, ack_position).await?;

                false
            }
            Ordering::Equal => {
                // Acked our whole buffer, no need to retransmit
                self.buffer.truncate(0);
                self.position.store(ack_position, atomic::Ordering::Relaxed);

                false
            }
        };

        let capacity = self
            .max_buffer_size
            .map(|max_buffer_size| max_buffer_size - self.buffer.len());
        if let Some(capacity) = capacity {
            if capacity > 0 {
                // Notify of capacity available
                if let Some(ref waker) = *self.last_waker_recv.borrow() {
                    // TODO: Ugly clone
                    waker.clone().wake()
                }
            }
        }

        // Notify of is_empty
        if !was_empty && self.buffer.is_empty() {
            if let Some(ref waker) = *self.last_waker_recv.borrow() {
                // TODO: Ugly clone
                waker.clone().wake()
            }
        }

        self.is_empty_send
            .send(self.buffer.is_empty())
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "the is_empty reader dropped"))?;

        Ok(misbehaved)
    }
}

#[derive(Debug)]
pub struct LrcpSessionWrite {
    buffer: Arc<Mutex<LrcpSessionBuffer>>,
    state: State,
    // TODO: Cell?
    last_waker_send: watch::Sender<Option<Waker>>,
    // TODO: Cell?
    is_empty_recv: watch::Receiver<bool>,
    // TODO: This is scary, possible mem leak due to circular Arc with socket owning the buffer
    socket: Arc<LrcpSocket>,
    session: LrcpSessionHandle,
}

impl LrcpSessionWrite {
    fn new(
        buffer: Arc<Mutex<LrcpSessionBuffer>>,
        last_waker_send: watch::Sender<Option<Waker>>,
        is_empty_recv: watch::Receiver<bool>,
        socket: Arc<LrcpSocket>,
        session: LrcpSessionHandle,
    ) -> Self {
        Self {
            buffer,
            state: State::Idle,
            last_waker_send,
            is_empty_recv,
            socket,
            session,
        }
    }

    fn write(&self, buf: Vec<u8>) -> impl Future<Output = io::Result<Option<usize>>> {
        let buffer = Arc::clone(&self.buffer);

        async move {
            // TODO: We're .to_owner() here so we might as well use a channel?
            buffer.lock().await.write(&buf).await
        }
    }

    fn send_close(&self) -> impl Future<Output = io::Result<()>> {
        let socket = Arc::clone(&self.socket);
        let LrcpSessionHandle(peer, session) = self.session;

        async move {
            // TODO: Abstract this a bit, here I shouldn't know about socket, message and possibly not even about peer/session
            socket
                .send_message(peer, &LrcpMessage::Close { session })
                .await
        }
    }

    fn handle_shutdown_finish() -> Poll<Result<(), io::Error>> {
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "Cannot shutdown an already shutdown LrcpSessionSend",
        )))
    }

    fn is_empty(&self) -> bool {
        *self.is_empty_recv.borrow()
    }

    fn store_waker(&self, cx: &mut Context<'_>) -> Result<(), std::io::Error> {
        self.last_waker_send
            .send(Some(cx.waker().clone()))
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "There was no waker listener"))
    }
}

impl AsyncWrite for LrcpSessionWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.store_waker(cx)?;

        match &mut self.state {
            State::Idle => {
                self.state = State::Writing(Box::pin(self.write(buf.to_owned())));
                let State::Writing(f ) = &mut self.state else {
                    unreachable!()
                };

                match f.as_mut().poll(cx) {
                    Poll::Ready(Ok(Some(len))) => {
                        self.state = State::Idle;
                        Poll::Ready(Ok(len))
                    }
                    Poll::Ready(Ok(None)) => {
                        self.state = State::Full;
                        Poll::Pending
                    }
                    Poll::Ready(Err(err)) => {
                        self.state = State::Idle;
                        Poll::Ready(Err(err))
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
            // TODO: Wakeup when capacity
            State::Full => Poll::Pending,
            State::Writing(f) => match f.as_mut().poll(cx) {
                Poll::Ready(Ok(Some(len))) => {
                    self.state = State::Idle;
                    Poll::Ready(Ok(len))
                }
                Poll::Ready(Ok(None)) => {
                    self.state = State::Full;
                    Poll::Pending
                }
                Poll::Ready(Err(err)) => {
                    self.state = State::Idle;
                    Poll::Ready(Err(err))
                }
                Poll::Pending => Poll::Pending,
            },
            // TODO: Wakeup when finished flushing
            State::Flushing => Poll::Pending,
            State::ShutdownSend(f) => {
                let polled = f.as_mut().poll(cx);

                if let Poll::Ready(Err(err)) = polled {
                    self.state = State::Shutdown;
                    return Poll::Ready(Err(err));
                }

                if !self.is_empty() {
                    self.state = State::ShutdownFlush;
                    return Poll::Pending;
                }

                self.state = State::Shutdown;
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Cannot write into a shutdown LrcpSessionSend",
                )))
            }
            // TODO: Wakeup when finished flushing
            State::ShutdownFlush => {
                if !self.is_empty() {
                    return Poll::Pending;
                }

                self.state = State::Shutdown;
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Cannot write into a shutdown LrcpSessionSend",
                )))
            }
            State::Shutdown => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Cannot write into a shutdown LrcpSessionSend",
            ))),
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        self.store_waker(cx)?;

        match &mut self.state {
            State::Idle => {
                if !self.is_empty() {
                    self.state = State::Flushing;
                    return Poll::Pending;
                }

                Poll::Ready(Ok(()))
            }
            State::Full => Poll::Pending,
            State::Writing(f) => {
                let value = ready!(f.as_mut().poll(cx))?;

                if !self.is_empty() {
                    self.state = State::Flushing;
                    return Poll::Pending;
                }

                Poll::Ready(Ok(()))
            }
            State::Flushing => Poll::Pending,
            State::ShutdownSend(f) => {
                let value = ready!(f.as_mut().poll(cx));

                if value.is_err() {
                    self.state = State::Shutdown;
                    Poll::Ready(value.map(|_| ()))
                } else if self.is_empty() {
                    self.state = State::Shutdown;
                    Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "Cannot write into a shutdown LrcpSessionSend",
                    )))
                } else {
                    self.state = State::ShutdownFlush;
                    Poll::Pending
                }
            }
            State::ShutdownFlush => {
                if self.is_empty() {
                    self.state = State::Shutdown;

                    Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "Cannot flush a shutdown LrcpSessionSend",
                    )))
                } else {
                    Poll::Pending
                }
            }
            State::Shutdown => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Cannot flush a shutdown LrcpSessionSend",
            ))),
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        self.store_waker(cx)?;

        match &mut self.state {
            State::Idle => {
                if self.is_empty() {
                    self.state = State::Shutdown;
                    Poll::Ready(Ok(()))
                } else {
                    self.state = State::ShutdownSend(Box::pin(self.send_close()));

                    let State::ShutdownSend(f) = &mut self.state else {
                        unreachable!()
                    };

                    let value = ready!(f.as_mut().poll(cx));

                    if value.is_err() {
                        self.state = State::Shutdown;
                        Poll::Ready(value)
                    } else if self.is_empty() {
                        self.state = State::Shutdown;
                        Poll::Ready(Ok(()))
                    } else {
                        self.state = State::ShutdownFlush;
                        Poll::Pending
                    }
                }
            }
            State::Full => Poll::Pending,
            State::Writing(f) => {
                let value = ready!(f.as_mut().poll(cx));

                if self.is_empty() {
                    Poll::Ready(value.map(|_| ()))
                } else {
                    self.state = State::ShutdownSend(Box::pin(self.send_close()));

                    let State::ShutdownSend(f) = &mut self.state else {
                        unreachable!()
                    };

                    let value = ready!(f.as_mut().poll(cx));

                    if value.is_err() {
                        self.state = State::Shutdown;
                        Poll::Ready(value)
                    } else if self.is_empty() {
                        self.state = State::Shutdown;
                        Poll::Ready(Ok(()))
                    } else {
                        self.state = State::ShutdownFlush;
                        Poll::Pending
                    }
                }
            }
            State::Flushing => Poll::Pending,
            State::ShutdownSend(f) => {
                let value = ready!(f.as_mut().poll(cx));

                if value.is_err() {
                    self.state = State::Shutdown;
                    Poll::Ready(value.map(|_| ()))
                } else if self.is_empty() {
                    self.state = State::Shutdown;
                    Poll::Ready(Ok(()))
                } else {
                    self.state = State::ShutdownFlush;
                    Poll::Pending
                }
            }
            State::ShutdownFlush => {
                if !self.is_empty() {
                    return Poll::Pending;
                }

                self.state = State::Shutdown;
                LrcpSessionWrite::handle_shutdown_finish()
            }
            State::Shutdown => LrcpSessionWrite::handle_shutdown_finish(),
        }
    }
}

enum State {
    Idle,
    Full,
    Writing(WritingFuture),
    Flushing,
    ShutdownSend(ShutdownSendFuture),
    ShutdownFlush,
    Shutdown,
}

impl Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Idle => f.write_str("State::Idle"),
            State::Full => f.write_str("State::Full"),
            State::Writing(_) => f.write_str("State::Writing"),
            State::Flushing => f.write_str("State::Flushing"),
            State::ShutdownSend(_) => f.write_str("State::ShutdownSend"),
            State::ShutdownFlush => f.write_str("State::ShutdownFlush"),
            State::Shutdown => f.write_str("State::Shutdown"),
        }
    }
}

// TODO: Retries and timeouts
// TODO: Handle acks

type WritingFuture = BoxFuture<'static, io::Result<Option<usize>>>;

type ShutdownSendFuture = BoxFuture<'static, io::Result<()>>;
