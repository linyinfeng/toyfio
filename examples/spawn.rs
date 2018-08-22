#![feature(futures_api)]

use std::io;
use log::{trace};
use futures::future;
use futures::Poll;

fn main() -> Result<(), io::Error> {
    pretty_env_logger::init();
    let future = future::poll_fn(|_context| {
        trace!("spawn another future in a future's poll function");
        let another = future::poll_fn(|_context| {
            trace!("another future spawned");
            Poll::Ready(())
        });
        toyfio::spawn(another);
        Poll::Ready(())
    });
	toyfio::run(future)
}
