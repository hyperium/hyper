## Examples of using hyper

Run examples with `cargo run --example example_name`.

### Available examples

* [`client`](client.rs) - A simple CLI http client that request the url passed in parameters and outputs the response content and details to the stdout, reading content chunk-by-chunk.

* [`echo`](echo.rs) - An echo server that copies POST request's content to the response content.

* [`hello`](hello.rs) - A simple server that returns "Hello World!" using a closure wrapped to provide a [`Service`](../src/service/service.rs).

* [`multi_server`](multi_server.rs) - A server that listens to two different ports, a different [`Service`](../src/service/service.rs) by port, spawning two [`futures`](../src/rt.rs).

* [`params`](params.rs) - A webserver that accept a form, with a name and a number, checks the parameters are presents and validates the input.

* [`send_file`](send_file.rs) - A server that sends back content of files using tokio_fs to read the files asynchronously.

* [`upgrades`](upgrades.rs) - A server and client demonstrating how to do HTTP upgrades (such as WebSockets or `CONNECT` tunneling).

* [`web_api`](web_api.rs) - A server consisting in a service that returns incoming POST request's content in the response in uppercase and a service that call that call the first service and includes the first service response in its own response.
