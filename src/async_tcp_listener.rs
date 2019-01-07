use crate::{AsyncTcpStream, REACTOR};
use futures::stream::Stream;
use futures::task::LocalWaker;
use futures::task::Poll;
use mio::net::TcpListener;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;

/// AsyncTcpListener is a wrapper of mio::net::TcpListener
#[derive(Debug)]
pub struct AsyncTcpListener(TcpListener);

impl AsyncTcpListener {
    /// Bind to the address and start listening
    pub fn bind(addr: &SocketAddr) -> Result<AsyncTcpListener, io::Error> {
        TcpListener::bind(addr).and_then(AsyncTcpListener::from_tcp_listener)
    }

    /// Convert `TcpListener` to `AsyncTcpListener`, register this
    /// `AsyncTcpListener` to `REACTOR`
    pub fn from_tcp_listener(listener: TcpListener) -> Result<AsyncTcpListener, io::Error> {
        REACTOR.with(|handle| handle.register(&listener))?;
        Ok(AsyncTcpListener(listener))
    }

    /// Get all incoming stream as a `Stream`
    pub fn incoming(self) -> Incoming {
        Incoming(self.0)
    }
}

/// A `Stream` for accepting connection from `AsyncTcpListener`
pub struct Incoming(TcpListener);

impl Stream for Incoming {
    type Item = Result<AsyncTcpStream, io::Error>;

    fn poll_next(self: Pin<&mut Self>, waker: &LocalWaker) -> Poll<Option<Self::Item>> {
        match self.0.accept() {
            Ok((stream, _)) => Poll::Ready(Some(AsyncTcpStream::from_tcp_stream(stream))),
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                match REACTOR.with(|reactor| reactor.reregister(&self.0, waker.clone(), mio::Ready::readable())) {
                    Ok(_) => Poll::Pending,
                    Err(err) => Poll::Ready(Some(Err(err))),
                }
            },
            Err(err) => Poll::Ready(Some(Err(err))),
        }
    }
}
