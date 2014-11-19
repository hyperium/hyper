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
fn hello(mut incoming: Incoming) {
    for conn in incoming {
        let (_, mut res) = conn.open().unwrap();
        *res.status_mut() = status::Ok;
        let mut res = res.start().unwrap();
        res.write(b"Hello World!");
        res.end().unwrap();
    }
}

fn main() {
    let server = Server::http(Ipv4Addr(127, 0, 0, 1), 1337);
    server.listen(hello).unwrap();
}
```

Client:

```rust
fn main() {
    // Creating an outgoing request.
    let mut req = Request::get(Url::parse("http://www.gooogle.com/").unwrap()).unwrap();

    // Setting a header.
    req.headers_mut().set(Connection(vec![Close]));

    // Start the Request, writing headers and starting streaming.
    let res = req.start().unwrap()
        // Send the Request.
        .send().unwrap()
        // Read the Response.
        .read_to_string().unwrap();

    println!("Response: {}", res);
}
```

## Scientific\* Benchmarks

[Client Bench:](./benches/client.rs)

```

running 3 tests
test bench_curl  ... bench:    298416 ns/iter (+/- 132455)
test bench_http  ... bench:    292725 ns/iter (+/- 167575)
test bench_hyper ... bench:    222819 ns/iter (+/- 86615)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured
```

[Mock Client Bench:](./benches/client_mock_tcp.rs)

```
running 3 tests
test bench_mock_curl  ... bench:     25254 ns/iter (+/- 2113)
test bench_mock_http  ... bench:     43585 ns/iter (+/- 1206)
test bench_mock_hyper ... bench:     27153 ns/iter (+/- 2227)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured
```


[Server Bench:](./benches/server.rs)

```
running 2 tests
test bench_http  ... bench:    296539 ns/iter (+/- 58861)
test bench_hyper ... bench:    233069 ns/iter (+/- 90194)

test result: ok. 0 passed; 0 failed; 0 ignored; 2 measured
```

\* No science was harmed in the making of this benchmark.

## License

[MIT](./LICENSE)

