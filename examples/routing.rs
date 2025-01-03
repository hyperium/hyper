#![deny(warnings)]

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use bytes::{Buf, Bytes};
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Incoming as IncomingBody, header, Method, Request, Response, StatusCode};
use tokio::net::{TcpListener, TcpStream};

#[path = "../benches/support/mod.rs"]
mod support;
use support::TokioIo;

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;
type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

static INDEX: &[u8] = b"<a href=\"test.html\">test.html</a>";
static INTERNAL_SERVER_ERROR: &[u8] = b"Internal Server Error";
static NOTFOUND: &[u8] = b"Not Found";
static POST_DATA: &str = r#"{"original": "data"}"#;
static URL: &str = "http://127.0.0.1:1337/json_api";

async fn client_request_response(
    _state: Arc<AtomicU64>,
    _req: Request<IncomingBody>,
) -> Result<Response<BoxBody>> {
    let req = Request::builder()
        .method(Method::POST)
        .uri(URL)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(POST_DATA)))
        .unwrap();

    let host = req.uri().host().expect("uri has no host");
    let port = req.uri().port_u16().expect("uri has no port");
    let stream = TcpStream::connect(format!("{}:{}", host, port)).await?;
    let io = TokioIo::new(stream);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection error: {:?}", err);
        }
    });

    let web_res = sender.send_request(req).await?;

    let res_body = web_res.into_body().boxed();

    Ok(Response::new(res_body))
}

async fn api_post_response(
    state: Arc<AtomicU64>,
    req: Request<IncomingBody>,
) -> Result<Response<BoxBody>> {
    // Aggregate the body...
    let whole_body = req.collect().await?.aggregate();
    // Decode as JSON...
    let mut data: serde_json::Value = serde_json::from_reader(whole_body.reader())?;
    // Change the JSON...
    data["test"] = serde_json::Value::from("test_value");
    // And respond with the new JSON.
    let json = serde_json::to_string(&data)?;
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(full(json))?;
    state.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(response)
}

async fn api_get_response(
    state: Arc<AtomicU64>,
    _req: Request<IncomingBody>,
) -> Result<Response<BoxBody>> {
    let responses = state.load(std::sync::atomic::Ordering::Relaxed).to_string();
    let data = vec!["foo", "bar", responses.as_str()];
    let res = match serde_json::to_string(&data) {
        Ok(json) => Response::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .body(full(json))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(full(INTERNAL_SERVER_ERROR))
            .unwrap(),
    };
    Ok(res)
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

type Routes<S> = HashMap<(Method, String), Box<dyn Handler<S>>>;

#[derive(Clone)]
struct Router<S> {
    routes: Arc<Routes<S>>,
    fallback: Pin<Arc<dyn Handler<S>>>,
    state: S,
}

impl<S> Router<S> {
    async fn handle_request(self, req: Request<IncomingBody>) -> Result<Response<BoxBody>> {
        // https://stackoverflow.com/questions/45786717 for a saner impl that is outside of the scope of this example
        if let Some(handler) = self
            .routes
            .get(&(req.method().clone(), req.uri().path().to_owned()))
        {
            handler.handle(self.state, req).await
        } else {
            self.fallback.handle(self.state, req).await
        }
    }
}

struct RouterBuilder<S> {
    routes: Routes<S>,
    fallback: Pin<Arc<dyn Handler<S>>>,
    state: S,
}

trait Handler<S>: Send + Sync {
    fn handle(
        &self,
        state: S,
        req: Request<IncomingBody>,
    ) -> Pin<Box<dyn Future<Output = Result<Response<BoxBody>>> + Send>>;
}

impl<F, S, Fut> Handler<S> for F
where
    F: Fn(S, Request<IncomingBody>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Response<BoxBody>>> + Send + 'static,
{
    fn handle(
        &self,
        state: S,
        req: Request<IncomingBody>,
    ) -> Pin<Box<dyn Future<Output = Result<Response<BoxBody>>> + Send>> {
        Box::pin(self(state, req))
    }
}

impl<S> RouterBuilder<S> {
    fn new(state: S) -> Self {
        Self {
            routes: HashMap::new(),
            fallback: Arc::pin(|_, _| {
                Box::pin(async {
                    Ok(Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(full(NOTFOUND))
                        .unwrap())
                })
            }),
            state,
        }
    }

    fn get(self, path: impl Into<String>, handler: impl Handler<S> + 'static) -> Self {
        self.route(Method::GET, path.into(), handler)
    }

    fn post(self, path: impl Into<String>, handler: impl Handler<S> + 'static) -> Self {
        self.route(Method::POST, path.into(), handler)
    }

    fn route(mut self, method: Method, path: String, handler: impl Handler<S> + 'static) -> Self {
        self.routes.insert((method, path), Box::new(handler));
        self
    }

    fn build(self) -> Router<S> {
        Router {
            routes: Arc::new(self.routes),
            fallback: self.fallback,
            state: self.state,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let router = RouterBuilder::new(Arc::new(AtomicU64::new(0)))
        .get("/", |_, _| async { Ok(Response::new(full(INDEX))) })
        .get("/index.html", |_, _| async {
            Ok(Response::new(full(INDEX)))
        })
        .get("/test.html", client_request_response)
        .post("/json_api", api_post_response)
        .get("/json_api", api_get_response)
        .build();

    let addr = SocketAddr::from(([127, 0, 0, 1], 1337));

    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let tmp_router = router.clone();

        tokio::task::spawn(async move {
            let service = service_fn(|req| tmp_router.clone().handle_request(req));

            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
