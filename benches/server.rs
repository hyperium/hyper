// You have to ctrl-C after running this benchmark, since there is no way to kill
// a rust-http server.

extern crate http;
extern crate hyper;
extern crate test;

use test::Bencher;
use std::io::net::ip::{SocketAddr, Ipv4Addr};

use http::server::Server;

static phrase: &'static [u8] = b"Benchmarking hyper vs others!";

fn request(url: hyper::Url) {
    let req = hyper::get(url).unwrap();
    req.send().unwrap().read_to_string().unwrap();
}

fn hyper_handle(mut incoming: hyper::server::Incoming) {
    for (_, mut res) in incoming {
        res.write(phrase).unwrap();
        res.end().unwrap();
    }
}

#[bench]
fn bench_hyper(b: &mut Bencher) {
    let server = hyper::Server::http(Ipv4Addr(127, 0, 0, 1), 0);
    let listener = server.listen(hyper_handle).unwrap();

    let url = hyper::Url::parse(format!("http://{}", listener.socket_addr).as_slice()).unwrap();
    b.iter(|| request(url.clone()));
    listener.close().unwrap();
}

static mut created_http: bool = false;

#[deriving(Clone)]
struct HttpServer;

impl Server for HttpServer {
    fn get_config(&self) -> http::server::Config {
        http::server::Config {
            bind_address: SocketAddr {
                ip: Ipv4Addr(127, 0, 0, 1),
                port: 4000
            }
        }
    }

    fn handle_request(&self, _: http::server::Request, res: &mut http::server::ResponseWriter) {
        res.write(phrase).unwrap();
    }
}

#[bench]
fn bench_http(b: &mut Bencher) {
    if unsafe { !created_http } { spawn(proc() { HttpServer.serve_forever() }); unsafe { created_http = true } }
    // Mega hack because there is no way to wait for serve_forever to start:
    std::io::timer::sleep(std::time::duration::Duration::seconds(1));

    let url = hyper::Url::parse("http://localhost:4000").unwrap();
    b.iter(|| request(url.clone()));
}

