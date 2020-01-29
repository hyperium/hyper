# Examples of using hyper

These examples show of how to do common tasks using `hyper`. You may also find the [Guides](https://hyper.rs/guides) helpful.

If you checkout this repository, you can run any of the examples `cargo run --example example_name`.

### Dependencies

Most of these examples use these dependencies:

```toml
[dependencies]
hyper = "0.13"
tokio = { version = "0.2", features = ["full"] }
pretty_env_logger = "0.3"
```

## Getting Started

### Clients

* [`client`](client.rs) - A simple CLI http client that request the url passed in parameters and outputs the response content and details to the stdout, reading content chunk-by-chunk.

* [`client_json`](client_json.rs) - A simple program that GETs some json, reads the body asynchronously, parses it with serde and outputs the result.

### Servers

* [`hello`](hello.rs) - A simple server that returns "Hello World!".

* [`echo`](echo.rs) - An echo server that copies POST request's content to the response content.

## Going Further

* [`gateway`](gateway.rs) - A server gateway (reverse proxy) that proxies to the `hello` service above.

* [`http_proxy`](http_proxy.rs) - A simple HTTP(S) proxy that handle and upgrade `CONNECT` requests and then proxy data between client and remote server.

* [`multi_server`](multi_server.rs) - A server that listens to two different ports, a different `Service` per port.

* [`params`](params.rs) - A webserver that accept a form, with a name and a number, checks the parameters are presents and validates the input.

* [`send_file`](send_file.rs) - A server that sends back content of files using tokio_fs to read the files asynchronously.

* [`single_threaded`](single_threaded.rs) - A server only running on 1 thread, so it can make use of `!Send` app state (like an `Rc` counter).

* [`state`](state.rs) - A webserver showing basic state sharing among requests. A counter is shared, incremented for every request, and every response is sent the last count.

* [`upgrades`](upgrades.rs) - A server and client demonstrating how to do HTTP upgrades (such as WebSockets).

* [`web_api`](web_api.rs) - A server consisting in a service that returns incoming POST request's content in the response in uppercase and a service that calls the first service and includes the first service response in its own response.
