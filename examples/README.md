# Examples of using hyper

These examples show how to do common tasks using `hyper`. You may also find the [Guides](https://hyper.rs/guides/1/) helpful.

If you checkout this repository, you can run any of the examples with the command:

 `cargo run --example {example_name} --features="full"`

### Dependencies

A complete list of dependencies used across these examples:

```toml
[dependencies]
hyper = { version = "1", features = ["full"] }
tokio = { version = "1", features = ["full"] }
pretty_env_logger = "0.5"
http-body-util = "0.1"
bytes = "1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
form_urlencoded = "1"
http = "1"
futures-util = { version = "0.3", default-features = false }
```

## Getting Started

### Clients

* [`client`](client.rs) - A simple CLI http client that requests the url passed in parameters and outputs the response content and details to the stdout, reading content chunk-by-chunk.

* [`client_json`](client_json.rs) - A simple program that GETs some json, reads the body asynchronously, parses it with serde and outputs the result.

### Servers

* [`hello`](hello.rs) - A simple server that returns "Hello World!".

* [`echo`](echo.rs) - An echo server that copies POST request's content to the response content.

## Going Further

* [`gateway`](gateway.rs) - A server gateway (reverse proxy) that proxies to the `hello` service above.

* [`graceful_shutdown`](graceful_shutdown.rs) - A server that has a timeout for incoming connections and does graceful connection shutdown.

* [`http_proxy`](http_proxy.rs) - A simple HTTP(S) proxy that handle and upgrade `CONNECT` requests and then proxy data between client and remote server.

* [`multi_server`](multi_server.rs) - A server that listens to two different ports, a different `Service` per port.

* [`params`](params.rs) - A webserver that accept a form, with a name and a number, checks the parameters are presents and validates the input.

* [`send_file`](send_file.rs) - A server that sends back content of files using tokio-util to read the files asynchronously.

* [`service_struct_impl`](service_struct_impl.rs) - A struct that manually implements the `Service` trait and uses a shared counter across requests.

* [`single_threaded`](single_threaded.rs) - A server only running on 1 thread, so it can make use of `!Send` app state (like an `Rc` counter).

* [`state`](state.rs) - A webserver showing basic state sharing among requests. A counter is shared, incremented for every request, and every response is sent the last count.

* [`upgrades`](upgrades.rs) - A server and client demonstrating how to do HTTP upgrades (such as WebSockets).

* [`web_api`](web_api.rs) - A server consisting in a service that returns incoming POST request's content in the response in uppercase and a service that calls the first service and includes the first service response in its own response.
