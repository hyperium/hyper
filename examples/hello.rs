//#![deny(warnings)]
extern crate hyper;
extern crate futures;
extern crate pretty_env_logger;
extern crate num_cpus;

use hyper::header::{ContentLength, ContentType};
use hyper::server::{Server, Service, Request, Response/*, HttpListener*/};

static PHRASE: &'static [u8] = b"Hello World!";

#[derive(Clone, Copy)]
struct Hello;

impl Service for Hello {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = ::futures::Finished<Response, hyper::Error>;
    fn call(&self, _req: Request) -> Self::Future {
        ::futures::finished(
            Response::new()
                .header(ContentLength(PHRASE.len() as u64))
                .header(ContentType::plaintext())
                .body(PHRASE)
        )
    }

}

fn main() {
    //env_logger::init().unwrap();
    pretty_env_logger::init();

    let (listening, server) = Server::http(&"127.0.0.1:3000".parse().unwrap()).unwrap()
        .standalone(|| Ok(Hello)).unwrap();

    println!("Listening on http://{}", listening);
    server.run();
    /*
    let listener = HttpListener::bind(&"127.0.0.1:3000".parse().unwrap()).unwrap();
    let mut handles = Vec::new();

    for _ in 0..num_cpus::get() {
        let listener = listener.try_clone().unwrap();
        handles.push(::std::thread::spawn(move || {
            Server::new(listener)
                .handle(|| Hello).unwrap();
        }));
    }
    println!("Listening on http://127.0.0.1:3000");

    for handle in handles {
        handle.join().unwrap();
    }
    */
}
