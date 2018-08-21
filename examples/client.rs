#![feature(futures_api,async_await,await_macro)]

use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::io;
use toyfio::AsyncTcpStream;
use futures::io::{AsyncWriteExt,AsyncReadExt};

async fn http_get(addr: &SocketAddr) -> Result<String, io::Error> {
	let mut conn = AsyncTcpStream::connect(addr)?;
	let _ = await!(conn.write_all(b"GET / HTTP/1.0\r\n\r\n"))?;
	let mut buf = Vec::new();
	await!(conn.read_to_end(&mut buf))?;
	Ok(String::from_utf8_lossy(&buf[..]).to_string())
}

async fn get_example() {
    let addr = "google.com:80".to_socket_addrs().unwrap().next().unwrap();
	let res = await!(http_get(&addr)).unwrap();
	println!("{}", res);
}

fn main() -> Result<(), io::Error> {
	pretty_env_logger::init();
	toyfio::run(get_example())
}
