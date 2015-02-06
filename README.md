# hyper

[![Build Status](https://travis-ci.org/hyperium/hyper.svg?branch=master)](https://travis-ci.org/hyperium/hyper)

A Modern HTTP library for Rust.

[Documentation](http://hyperium.github.io/hyper)

## Overview

Hyper is a fast, modern HTTP implementation written in and for Rust. It
is a low-level typesafe abstraction over raw HTTP, providing an elegant
layer over "stringly-typed" HTTP.

Hyper offers both an HTTP/S client and HTTP server which can be used to drive
complex web applications written entirely in Rust.

The documentation is located at [http://hyperium.github.io/hyper](http://hyperium.github.io/hyper).

__WARNING: Hyper is still under active development. The API is still changing
in non-backwards-compatible ways without warning.__

## Example

Hello World Server:

```rust
fn hello(_: Request, res: Response<Fresh>) {
    *res.status_mut() = status::Ok;
    let mut res = res.start().unwrap();
    res.write(b"Hello World!");
    res.end().unwrap();
}

fn main() {
    let server = Server::http(Ipv4Addr(127, 0, 0, 1), 1337);
    server.listen(hello).unwrap();
}
```

Client:

```rust
fn main() {
    // Create a client.
    let mut client = Client::new();

    // Creating an outgoing request.
    let res = client.get("http://www.gooogle.com/")
        // set a header
        .header(Connection(vec![Close]))
        // let 'er go!
        .send().unwrap();

    // Read the Response.
    let body = res.read_to_string().unwrap();

    println!("Response: {}", body);
}
```

## License

[MIT](./LICENSE)

