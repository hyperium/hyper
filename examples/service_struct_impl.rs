use hyper::service::Service;
use hyper::{Body, Request, Response, Server};

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

type Counter = i32;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr).serve(MakeSvc { counter: 81818 });
    println!("Listening on http://{}", addr);

    server.await?;
    Ok(())
}

struct Svc {
    counter: Counter,
}

impl Service<Request<Body>> for Svc {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        fn mk_response(s: String) -> Result<Response<Body>, hyper::Error> {
            Ok(Response::builder().body(Body::from(s)).unwrap())
        }

        let res = match req.uri().path() {
            "/" => mk_response(format!("home! counter = {:?}", self.counter)),
            "/posts" => mk_response(format!("posts, of course! counter = {:?}", self.counter)),
            "/authors" => mk_response(format!(
                "authors extraordinare! counter = {:?}",
                self.counter
            )),
            // Return the 404 Not Found for other routes, and don't increment counter.
            _ => return Box::pin(async { mk_response("oh no! not found".into()) }),
        };

        if req.uri().path() != "/favicon.ico" {
            self.counter += 1;
        }

        Box::pin(async { res })
    }
}

struct MakeSvc {
    counter: Counter,
}

impl<T> Service<T> for MakeSvc {
    type Response = Svc;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let counter = self.counter.clone();
        let fut = async move { Ok(Svc { counter }) };
        Box::pin(fut)
    }
}
