#![feature(async_await)]
#![deny(warnings)]
extern crate hyper;
extern crate pretty_env_logger;
extern crate serde_json;

use hyper::{Body, Chunk, Client, Method, Request, Response, Server, StatusCode, header};
use hyper::client::HttpConnector;
use hyper::service::{service_fn, make_service_fn};
use futures_util::{TryStreamExt};

type GenericError = Box<dyn std::error::Error + Send + Sync>;

static NOTFOUND: &[u8] = b"Not Found";
static URL: &str = "http://127.0.0.1:1337/json_api";
static INDEX: &[u8] = b"<a href=\"test.html\">test.html</a>";
static POST_DATA: &str = r#"{"original": "data"}"#;

async fn client_request_response(client: &Client<HttpConnector>)
    -> Result<Response<Body>, GenericError>
{
     let req = Request::builder()
         .method(Method::POST)
         .uri(URL)
         .header(header::CONTENT_TYPE, "application/json")
         .body(POST_DATA.into())
         .unwrap();

     let web_res = client.request(req).await?;
    // Compare the JSON we sent (before) with what we received (after):
    let body = Body::wrap_stream(web_res.into_body().map_ok(|b| {
        Chunk::from(format!("<b>POST request body</b>: {}<br><b>Response</b>: {}",
                            POST_DATA,
                            std::str::from_utf8(&b).unwrap()))
    }));

    Ok(Response::new(body))
}

async fn api_post_response(req: Request<Body>)
    -> Result<Response<Body>, GenericError>
{
    // A web api to run against
    let entire_body = req.into_body().try_concat().await?;
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
}

async fn api_get_response() -> Result<Response<Body>, GenericError> {
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
    Ok(res)
}

async fn response_examples(req: Request<Body>, client: &Client<HttpConnector>)
    -> Result<Response<Body>, GenericError>
{
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            let body = Body::from(INDEX);
            Ok(Response::new(body))
        },
        (&Method::GET, "/test.html") => {
           client_request_response(client).await
        },
        (&Method::POST, "/json_api") => {
            api_post_response(req).await
        },
        (&Method::GET, "/json_api") => {
            api_get_response().await
        }
        _ => {
            // Return 404 not found response.
            let body = Body::from(NOTFOUND);
            Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(body)
                        .unwrap())
        }
    }
}

#[hyper::rt::main]
async fn main() -> Result<(), GenericError> {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1337".parse().unwrap();

    // Share a `Client` with all `Service`s
    let client = Client::new();

    let new_service = make_service_fn(move |_| {
        // Move a clone of `client` into the `service_fn`.
        let client = client.clone();
        async {
            Ok::<_, GenericError>(service_fn(move |req| {
                response_examples(req, &client)
            }))
        }
    });

    let server = Server::bind(&addr)
        .serve(new_service);

    println!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}
