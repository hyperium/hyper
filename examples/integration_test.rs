#![deny(warnings)]
#![warn(rust_2018_idioms)]
use hyper::service::{make_service_fn, service_fn};
use hyper::{body, Body, Client, Request, Response, Server, StatusCode};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, Error>;

// Server code for testing. If you're writing an integration test for a server you would use your
// real server instead of this.
async fn test_service(request: Request<Body>) -> Result<Response<Body>> {
    match request.uri().path() {
        "/my-api" => Ok(Response::builder().body(Body::from("Hello world"))?),
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())?),
    }
}

// Note: In an integration test (in the `tests` directory), you would use `#[tokio::test]` instead
// of `#[tokio::main]`. Here we need to use `#[tokio::main]` because it's in an example.
#[tokio::main]
async fn main() -> Result<()> {
    // Set up test server. We bind to port 0 which means use an available port, and after starting
    // the server we get the bound address to run the test against.
    let make_svc = make_service_fn(|_conn| async { Ok::<_, Error>(service_fn(test_service)) });
    let server = Server::bind(&([127, 0, 0, 1], 0).into()).serve(make_svc);
    let base_url = format!("http://{}", server.local_addr());

    // Run server in background. The server will automatically terminate once the test is done.
    tokio::spawn(server);

    // Client code for testing.. If you're writing an integration test for a client you would use
    // your real client here and set up a test server with expected responses.
    let uri = format!("{}/my-api", base_url).parse()?;
    let client = Client::new();
    let response = client.get(uri).await?;

    let bytes = body::to_bytes(response).await?;
    let text = std::str::from_utf8(bytes.as_ref())?;

    assert_eq!("Hello world", text);

    Ok(())
}
