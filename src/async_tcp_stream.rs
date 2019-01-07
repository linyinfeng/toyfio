use futures::{
    io::{AsyncRead, AsyncWrite},
    task::LocalWaker,
    Poll,
};
use mio::net::TcpStream;
use std::{
    io::{self, Read, Write},
    net::SocketAddr,
};

use crate::REACTOR;

/// `AsyncTcpStream` is a wrapper of `mio::net::TcpStream`
#[derive(Debug)]
pub struct AsyncTcpStream(TcpStream);

impl AsyncTcpStream {
    /// Create a new `AsyncTcpStream`.
    ///
    /// The result of connect has no need to be a future, here are reasons:
    /// - `mio::net::TcpStream` will perform async connect for the returned
    ///   `TcpStream`.
    /// - The returned `TcpStream` automatically become pending and wait connect
    ///   when call write methods of AsyncTcpStream.
    pub fn connect(addr: &SocketAddr) -> Result<AsyncTcpStream, io::Error> {
        match TcpStream::connect(addr) {
            Ok(stream) => AsyncTcpStream::from_tcp_stream(stream),
            Err(e) => Err(e),
        }
    }

    /// Convert `TcpStream` to `AsyncTcpStream`, register this `AsyncTcpStream`
    /// to `REACTOR`
    pub fn from_tcp_stream(stream: TcpStream) -> Result<AsyncTcpStream, io::Error> {
        REACTOR.with(|handle| handle.register(&stream))?;
        Ok(AsyncTcpStream(stream))
    }
}

impl AsyncRead for AsyncTcpStream {
    fn poll_read(&mut self, waker: &LocalWaker, buf: &mut [u8]) -> Poll<Result<usize, io::Error>> {
        match self.0.read(buf) {
            Ok(len) => Poll::Ready(Ok(len)),
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                match REACTOR.with(|handle| handle.reregister(&self.0, waker.clone(), mio::Ready::readable())) {
                    Ok(_) => Poll::Pending,
                    Err(e) => Poll::Ready(Err(e)),
                }
            },
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

impl AsyncWrite for AsyncTcpStream {
    fn poll_write(&mut self, waker: &LocalWaker, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        match self.0.write(buf) {
            Ok(len) => Poll::Ready(Ok(len)),
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                match REACTOR.with(|handle| handle.reregister(&self.0, waker.clone(), mio::Ready::writable())) {
                    Ok(_) => Poll::Pending,
                    Err(e) => Poll::Ready(Err(e)),
                }
            },
            Err(err) => Poll::Ready(Err(err)),
        }
    }

    fn poll_flush(&mut self, _waker: &LocalWaker) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(&mut self, _waker: &LocalWaker) -> Poll<Result<(), io::Error>> {
        // Socket will be closed when this AsyncTcpStream was dropped
        Poll::Ready(Ok(()))
    }
}
