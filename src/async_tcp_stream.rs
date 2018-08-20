use std::{
    net::SocketAddr,
    io::{self, Read, Write}
};
use mio::net::TcpStream;
use futures::{
    Poll,
    task::Context,
    io::{AsyncRead, AsyncWrite}
};

use crate::REACTOR;

/// `AsyncTcpStream` is a wrapper of `mio::net::TcpStream`
#[derive(Debug)]
pub struct AsyncTcpStream(TcpStream);

impl AsyncTcpStream {
    /// Create a new `AsyncTcpStream`.
    ///
    /// The result of connect has no need to be a future, here are reasons:
    /// - `mio::net::TcpStream` will perform async connect for the returned `TcpStream`.
    /// - The returned `TcpStream` automatically become pending and wait connect when call write
    ///   methods of AsyncTcpStream.
    pub fn connect(addr: &SocketAddr) -> Result<AsyncTcpStream, io::Error> {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                REACTOR.with(|handle| handle.register(&stream))?;
                Ok(AsyncTcpStream(stream))
            },
            Err(e) => Err(e),
        }
    }
}

impl AsyncRead for AsyncTcpStream {
    fn poll_read(&mut self, cx: &mut Context, buf: &mut [u8]) -> Poll<Result<usize, io::Error>> {
        let waker = cx.local_waker().clone();
        match self.0.read(buf) {
            Ok(len) => Poll::Ready(Ok(len)),
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                match REACTOR.with(|handle| handle.reregister(&self.0, waker, mio::Ready::readable())) {
                    Ok(_) => Poll::Pending,
                    Err(e) => Poll::Ready(Err(e)),
                }
            },
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

impl AsyncWrite for AsyncTcpStream {
    fn poll_write(&mut self, cx: &mut Context, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        let waker = cx.local_waker().clone();
        match self.0.write(buf) {
            Ok(len) => Poll::Ready(Ok(len)),
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                match REACTOR.with(|handle| handle.reregister(&self.0, waker, mio::Ready::writable())) {
                    Ok(_) => Poll::Pending,
                    Err(e) => Poll::Ready(Err(e)),
                }
            },
            Err(err) => Poll::Ready(Err(err)),
        }
    }

    fn poll_flush(&mut self, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(&mut self, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
        // Socket will be closed when this AsyncTcpStream was dropped
        Poll::Ready(Ok(()))
    }
}