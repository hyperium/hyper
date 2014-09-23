# hyper

[![Build Status](https://travis-ci.org/hyperium/hyper.svg?branch=master)](https://travis-ci.org/hyperium/hyper)

A Modern HTTP library for Rust.

[Documentation](http://hyperium.github.io/hyper)

## Overview

Hyper is a fast, modern HTTP implementation written in and for Rust. It
is a low-level typesafe abstraction over raw HTTP, providing an elegant
layer over "stringly-typed" HTTP.

Hyper offers both an HTTP/S client an HTTP server which can be used to drive
complex web applications written entirely in Rust.

The documentation is located at [http://hyperium.github.io/hyper](http://hyperium.github.io/hyper).

## Example

Hello World Server:

```rust
fn hello(mut incoming: Incoming) {
    for (_, mut res) in incoming {
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
    let mut req = Request::get(Url::parse("http://www.gooogle.com/")).unwrap();

    // Setting a header.
    req.headers_mut().set(Foo);

    // Start the Request, writing headers and starting streaming.
    let res = req.start().unwrap()
        // Send the Request.
        .send().unwrap()
        // Read the Response.
        .read_to_string().unwrap()

    println!("Response: {}", res);
}
```

## Scientific\* Benchmarks

[Client Bench:](./benches/client.rs)

```

running 3 tests
test bench_curl  ... bench:   1696689 ns/iter (+/- 540497)
test bench_http  ... bench:   2222778 ns/iter (+/- 1159060)
test bench_hyper ... bench:   1435613 ns/iter (+/- 359384)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured
```

[Mock Client Bench:](./benches/client_mock_tcp.rs)

```
running 3 tests
test bench_mock_curl  ... bench:    329240 ns/iter (+/- 50413)
test bench_mock_http  ... bench:     61291 ns/iter (+/- 19253)
test bench_mock_hyper ... bench:     54458 ns/iter (+/- 15792)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured
```


[Server Bench:](./benches/server.rs)

```
running 3 tests
test bench_curl  ... bench:    234539 ns/iter (+/- 22228)
test bench_http  ... bench:    290370 ns/iter (+/- 69179)
test bench_hyper ... bench:    224482 ns/iter (+/- 95197)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured
```

\* No science was harmed in the making of this benchmark.

## License

[MIT](./LICENSE)

