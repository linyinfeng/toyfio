# Toyfio - A toy Rust I/O library with Mio and Futures 0.3 preview

It's a toy I/O library built with Futures 0.3 preview and Mio.

## Aim

The project just try to find out the simplest way to implement a high-performance toy asynchronous network I/O library based on Mio with futures-rs 0.3.0.alpha.

So, all choices the project has made are all experimental.

## Feature

This library is directly built on Mio as very thin wrapper of mio.

Now it only support `AsyncTcpStream` and `AsyncTcpListener`.

## Usage

Read the source code. And to run the example, use rust nightly toolchain, and just run:

```shell
cargo +nightly run --example client
```

## Todo

- [x] A `TcpListener`
- [x] Replace slab with a `LeakStorage`
- [ ] Redesign the `InnerWaker`
- [ ] Continuously update to working with newest futures preview
