#![feature(futures_api,async_await,await_macro)]

use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::io;
use fio::AsyncTcpStream;
use futures::io::{AsyncWriteExt,AsyncReadExt};

async fn http_get(addr: &SocketAddr) -> Result<String, io::Error> {
	let mut conn = AsyncTcpStream::connect(addr)?;
	let _ = await!(conn.write_all(b"GET / HTTP/1.0\r\n\r\n"))?;
	let mut buf = vec![0;1024];
	let len = await!(conn.read(&mut buf))?;
	Ok(String::from_utf8_lossy(&buf[..len]).to_string())
}

async fn get_baidu() {
    let addr = "google.com:80".to_socket_addrs().unwrap().next().unwrap();
	let res = await!(http_get(&addr)).unwrap();
	println!("{}", res);
}

fn main() -> Result<(), io::Error> {
	pretty_env_logger::init();
	fio::run(get_baidu())
}
