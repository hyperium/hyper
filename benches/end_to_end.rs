#![feature(test)]

extern crate futures;
extern crate hyper;
extern crate tokio_core;

extern crate test;

use futures::{Future, Stream};
use tokio_core::reactor::Core;

use hyper::header::{ContentLength, ContentType};
use hyper::server::{Service, Request, Response};


#[bench]
fn one_request_at_a_time(b: &mut test::Bencher) {
    extern crate pretty_env_logger;
    let _ = pretty_env_logger::init();
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let addr = hyper::Server::http(&"127.0.0.1:0".parse().unwrap(), &handle).unwrap()
        .handle(|| Ok(Hello), &handle).unwrap();

    let mut client = hyper::Client::new(&handle);

    let url: hyper::Url = format!("http://{}/get", addr).parse().unwrap();

    b.bytes = 160;
    b.iter(move || {
        let work = client.get(url.clone()).and_then(|res| {
            res.body().for_each(|_chunk| {
                Ok(())
            })
        });

        core.run(work).unwrap();
    });
}

static PHRASE: &'static [u8] = b"Hello, World!";

#[derive(Clone, Copy)]
struct Hello;

impl Service for Hello {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = ::futures::Finished<Response, hyper::Error>;
    fn call(&mut self, _req: Request) -> Self::Future {
        ::futures::finished(
            Response::new()
                .with_header(ContentLength(PHRASE.len() as u64))
                .with_header(ContentType::plaintext())
                .with_body(PHRASE)
        )
    }

}
