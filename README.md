# Toyfio -  A toy Rust I/O library with Mio and Futures 0.3 preview

It's a toy I/O library built with Futures 0.3 preview and Mio.

## Feature

Now it only support `spawn`, `run` and `AsyncTcpStream`.

This library is directly built on Mio. The reactor only contains an extra counter to support auto stopping of event loop.

## Note

This implementation should not to be referenced as an example for it's using unsafe to bypass some restrictions of futures-rs and its safety may not be guaranteed.

## Usage

Read the source code. And to run the example, use rust nightly toolchain, and just run:

```shell
cargo +nightly run --example client
```

## Todo

- Consider changing memory leak to using [slab](https://crates.io/crates/slab)
- A `TcpListener`