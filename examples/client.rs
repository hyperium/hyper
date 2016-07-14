#![deny(warnings)]
extern crate hyper;

extern crate env_logger;

use std::env;
use std::io;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::{Client, Request, Response, DefaultTransport as HttpStream};
use hyper::header::Connection;
use hyper::{Decoder, Encoder, Next};

#[derive(Debug)]
struct Dump(mpsc::Sender<()>);

impl Drop for Dump {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn read() -> Next {
    Next::read().timeout(Duration::from_secs(10))
}

impl hyper::client::Handler<HttpStream> for Dump {
    fn on_request(&mut self, req: &mut Request) -> Next {
        req.headers_mut().set(Connection::close());
        read()
    }

    fn on_request_writable(&mut self, _encoder: &mut Encoder<HttpStream>) -> Next {
        read()
    }

    fn on_response(&mut self, res: Response) -> Next {
        println!("Response: {}", res.status());
        println!("Headers:\n{}", res.headers());
        read()
    }

    fn on_response_readable(&mut self, decoder: &mut Decoder<HttpStream>) -> Next {
        match io::copy(decoder, &mut io::stdout()) {
            Ok(0) => Next::end(),
            Ok(_) => read(),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => Next::read(),
                _ => {
                    println!("ERROR:example: {}", e);
                    Next::end()
                }
            }
        }
    }

    fn on_error(&mut self, err: hyper::Error) -> Next {
        println!("ERROR:example: {}", err);
        Next::remove()
    }
}

fn main() {
    env_logger::init().unwrap();

    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return;
        }
    };

    let (tx, rx) = mpsc::channel();
    let client = Client::new().expect("Failed to create a Client");
    client.request(url.parse().unwrap(), Dump(tx)).unwrap();

    // wait till done
    let _  = rx.recv();
    client.close();
}
