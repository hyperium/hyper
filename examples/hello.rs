#![deny(warnings)]
extern crate hyper;
extern crate env_logger;
extern crate num_cpus;

use hyper::{Decoder, Encoder, Next, HttpStream};
use hyper::server::{Server, Handler, Request, Response, HttpListener};

static PHRASE: &'static [u8] = b"Hello World!";

struct Hello;

impl Handler<HttpStream> for Hello {
    fn on_request(&mut self, _: Request<HttpStream>) -> Next {
        Next::write()
    }
    fn on_request_readable(&mut self, _: &mut Decoder<HttpStream>) -> Next {
        Next::write()
    }
    fn on_response(&mut self, response: &mut Response) -> Next {
        use hyper::header::ContentLength;
        response.headers_mut().set(ContentLength(PHRASE.len() as u64));
        Next::write()
    }
    fn on_response_writable(&mut self, encoder: &mut Encoder<HttpStream>) -> Next {
        let n = encoder.write(PHRASE).unwrap();
        debug_assert_eq!(n, PHRASE.len());
        Next::end()
    }
}

fn main() {
    env_logger::init().unwrap();
 
    let listener = HttpListener::bind(&"127.0.0.1:3000".parse().unwrap()).unwrap();
    let mut handles = Vec::new();

    for _ in 0..num_cpus::get() {
        let listener = listener.try_clone().unwrap();
        handles.push(::std::thread::spawn(move || {
            Server::new(listener)
                .handle(|_| Hello).unwrap();
        }));
    }
    println!("Listening on http://127.0.0.1:3000");

    for handle in handles {
        handle.join().unwrap();
    }
}
