extern crate curl;
extern crate http;
extern crate hyper;

extern crate test;

use std::io::IoResult;
use std::time::Duration;
use std::io::timer::sleep;
use std::io::net::ip::Ipv4Addr;
use std::sync::{Once, ONCE_INIT};
use hyper::server::{Request, Response, Server};

static mut SERVER: Once = ONCE_INIT;

fn listen() {
    unsafe {
        SERVER.doit(|| {
            let server = Server::http(Ipv4Addr(127, 0, 0, 1), 1337);
            let listening = server.listen(handle).unwrap();
            spawn(proc() {
                sleep(Duration::seconds(20));
                listening.close().unwrap();
            });
        })
    }
}

fn handle(_req: Request, mut res: Response) -> IoResult<()> {
    try!(res.write(b"Benchmarking hyper vs others!"));
    res.end()
}


#[bench]
fn bench_curl(b: &mut test::Bencher) {
    listen();
    b.iter(|| {
        curl::http::handle().get("http://127.0.0.1:1337/").exec().unwrap()
    });
}

#[bench]
fn bench_hyper(b: &mut test::Bencher) {
    listen();
    b.iter(|| {
        hyper::get(hyper::Url::parse("http://127.0.0.1:1337/").unwrap()).unwrap()
            .send().unwrap()
            .read_to_string().unwrap()
    });
}

#[bench]
fn bench_http(b: &mut test::Bencher) {
    listen();
    b.iter(|| {
        let req: http::client::RequestWriter = http::client::RequestWriter::new(
            http::method::Get,
            hyper::Url::parse("http://127.0.0.1:1337/").unwrap()
        ).unwrap();
        // cant unwrap because Err contains RequestWriter, which does not implement Show
        let mut res = match req.read_response() {
            Ok(res) => res,
            Err(..) => fail!("http response failed")
        };
        res.read_to_string().unwrap();
    })
}
