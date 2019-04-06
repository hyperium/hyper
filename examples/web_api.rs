#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate serde_json;

use futures::{future, Future, Stream};

use hyper::{Body, Chunk, Client, Method, Request, Response, Server, StatusCode, header};
use hyper::client::HttpConnector;
use hyper::service::service_fn;

static NOTFOUND: &[u8] = b"Not Found";
static URL: &str = "http://127.0.0.1:1337/json_api";
static INDEX: &[u8] = b"<a href=\"test.html\">test.html</a>";
static POST_DATA: &str = r#"{"original": "data"}"#;

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<Future<Item=Response<Body>, Error=GenericError> + Send>;

fn client_request_response(client: &Client<HttpConnector>) -> ResponseFuture {
     let req = Request::builder()
         .method(Method::POST)
         .uri(URL)
         .header(header::CONTENT_TYPE, "application/json")
         .body(POST_DATA.into())
         .unwrap();

     Box::new(client.request(req).from_err().map(|web_res| {
         // Compare the JSON we sent (before) with what we received (after):
         let body = Body::wrap_stream(web_res.into_body().map(|b| {
             Chunk::from(format!("<b>POST request body</b>: {}<br><b>Response</b>: {}",
                                 POST_DATA,
                                 std::str::from_utf8(&b).unwrap()))
         }));

         Response::new(body)
     }))
}

fn api_post_response(req: Request<Body>) -> ResponseFuture {
    // A web api to run against
    Box::new(req.into_body()
        .concat2() // Concatenate all chunks in the body
        .from_err()
        .and_then(|entire_body| {
            // TODO: Replace all unwraps with proper error handling
            let str = String::from_utf8(entire_body.to_vec())?;
            let mut data : serde_json::Value = serde_json::from_str(&str)?;
            data["test"] = serde_json::Value::from("test_value");
            let json = serde_json::to_string(&data)?;
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json))?;
            Ok(response)
        })
    )
}

fn api_get_response() -> ResponseFuture {
    let data = vec!["foo", "bar"];
    let res = match serde_json::to_string(&data) {
        Ok(json) => {
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json))
                .unwrap()
        }
        Err(_) => {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Server Error"))
                .unwrap()
        }
    };

    Box::new(future::ok(res))
}

fn response_examples(req: Request<Body>, client: &Client<HttpConnector>) -> ResponseFuture {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            let body = Body::from(INDEX);
            Box::new(future::ok(Response::new(body)))
        },
        (&Method::GET, "/test.html") => {
           client_request_response(client)
        },
        (&Method::POST, "/json_api") => {
            api_post_response(req)
        },
        (&Method::GET, "/json_api") => {
            api_get_response()
        }
        _ => {
            // Return 404 not found response.
            let body = Body::from(NOTFOUND);
            Box::new(future::ok(Response::builder()
                                         .status(StatusCode::NOT_FOUND)
                                         .body(body)
                                         .unwrap()))
        }
    }
}

fn main() {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1337".parse().unwrap();

    hyper::rt::run(future::lazy(move || {
        // Share a `Client` with all `Service`s
        let client = Client::new();

        let new_service = move || {
            // Move a clone of `client` into the `service_fn`.
            let client = client.clone();
            service_fn(move |req| {
                response_examples(req, &client)
            })
        };

        let server = Server::bind(&addr)
            .serve(new_service)
            .map_err(|e| eprintln!("server error: {}", e));

        println!("Listening on http://{}", addr);

        server
    }));
}
