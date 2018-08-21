# Toyfio -  A toy Rust I/O library with Mio and Futures 0.3 preview

It's a toy I/O library built with Futures 0.3 preview and Mio.

## Feature

This library is directly built on Mio as very thin wrapper of mio.

Now it only support `spawn`, `run` and `AsyncTcpStream`.

## Usage

Read the source code. And to run the example, use rust nightly toolchain, and just run:

```shell
cargo +nightly run --example client
```

## Todo

- [x] Consider changing memory leak to using [slab](https://crates.io/crates/slab)
- A `TcpListener`