#![feature(futures_api, async_await, await_macro)]

use futures::prelude::*;
use log::error;
use std::io;
use std::net::SocketAddr;
use toyfio::{AsyncTcpListener, AsyncTcpStream};

async fn echo(stream: Result<AsyncTcpStream, io::Error>) -> Result<(), io::Error> {
    let mut stream = stream?;
    let mut buf = [0; 40960];
    loop {
        let len = await!(stream.read(&mut buf[..]))?;
        // debug!("read {} bytes", len); // Enable log will significantly reduce
        // performance
        if len == 0 {
            break
        }
        await!(stream.write_all(&buf[..len]))?;
    }
    Ok(())
}

async fn echo_server(addr: SocketAddr) {
    let listener = AsyncTcpListener::bind(&addr).unwrap();
    await!(listener.incoming().for_each(async move |stream| {
        let done = echo(stream).unwrap_or_else(|err| {
            error!("error occurred when echo: {:?}", err);
        });
        toyfio::spawn(done);
    }));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let addr = "127.0.0.1:9999".parse()?;
    toyfio::run(echo_server(addr))?;
    Ok(())
}
