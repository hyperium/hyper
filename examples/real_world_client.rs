#![deny(warnings)]
/// client with gzip and https support.
/// Usage:
/// cargo run --example real_world_client https://httpbin.org/gzip

extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate tokio_core;
extern crate flate2;
extern crate pretty_env_logger;

use std::env;
use std::io::Read;
use hyper::header::{ContentEncoding, AcceptEncoding, Encoding, qitem};
use hyper::Get;
use hyper::client::Request;

use futures::Future;
use futures::stream::Stream;

use flate2::read::GzDecoder;

fn main() {
    pretty_env_logger::init().unwrap();

    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return;
        }
    };

    let url = url.parse::<hyper::Uri>().unwrap();

    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();

    let client = hyper::Client::configure()
        .connector((hyper_tls::HttpsConnector::new(4, &handle)).unwrap())
        .build(&handle);

    let mut req = Request::new(Get, url);
    {
        let headers = req.headers_mut();
        headers.set(AcceptEncoding(vec![qitem(Encoding::Gzip)]))
    }

    let work = client
        .request(req)
        .and_then(|res| {
            println!("Response: {}", res.status());
            println!("Headers: \n{}", res.headers());

            let status = res.status();
            let headers = res.headers().clone();

            res.body()
                .fold((status, headers, Vec::new()),
                      |(status, headers, mut acc), chunk| {
                          acc.extend_from_slice(chunk.as_ref());
                          Ok::<_, hyper::Error>((status, headers, acc))
                      })
        })
        .map(|(_status, headers, acc)| {

            if let Some(&ContentEncoding(ref ce)) = headers.get() {
                println!("ContentEncoding: {:?}", ce);
                if ce == &[Encoding::Gzip] {
                    println!("gzip detected, uncompressing ...");
                    let mut decoder = GzDecoder::new(&*acc).unwrap();

                    let mut buffer = Vec::new();
                    let _ = decoder.read_to_end(&mut buffer);
                    println!("uncompressed response: {}",
                             String::from_utf8_lossy(&*buffer));
                    println!("original size: {:?}, uncompressed size: {:?} ratio: {:?}",
                             acc.len(),
                             buffer.len(),
                             acc.len() as f32 / buffer.len() as f32);
                    return;
                }
            }

            println!("no compressing: {:?}", String::from_utf8_lossy(&*acc));
        });

    core.run(work).unwrap();
}
