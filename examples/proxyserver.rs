extern crate tokio_core;
extern crate tokio_io;
extern crate futures;
extern crate hyper;

use std::net::SocketAddr;
use tokio_core::reactor::{Core, Handle};
use tokio_core::net::TcpListener;
use futures::Future;
use futures::stream::Stream;
use hyper::client::{self, Client};
use hyper::server::{self, Service, Http};
use hyper::error::Error;


struct Proxy {
    handle: Handle,
}

impl Service for Proxy {
    type Request = server::Request;
    type Response = server::Response;
    type Error = Error;
    type Future = Box<Future<Item=Self::Response, Error = Error>>;

    fn call(&self, req: server::Request) -> Self::Future {
        let method = req.method().clone();
        let uri = req.uri().clone();
        let mut client_req = client::Request::new(method, uri);
        client_req.headers_mut().extend(req.headers().iter());
        client_req.set_body(req.body());

        let client = Client::new(&self.handle);
        let resp = client.request(client_req)
                         .then(move |result| {
                             match result {
                                 Ok(client_resp) => {
                                     Ok(server::Response::new()
                                            .with_status(client_resp.status())
                                            .with_headers(client_resp.headers().clone())
                                            .with_body(client_resp.body()))
                                 }
                                 Err(e) => {
                                     println!("{:?}", &e);
                                     Err(e)
                                 }
                             }
                         });
        Box::new(resp)
    }
}


fn main() {
    let srv_addr: SocketAddr = "127.0.0.1:8888".parse().unwrap();

    let http = Http::new();
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&srv_addr, &handle).unwrap();
    let server = listener.incoming()
                         .for_each(|(sock, addr)| {
                             let service = Proxy { handle: handle.clone() };
                             http.bind_connection(&handle, sock, addr, service);
                             Ok(())
                         });

    core.run(server).unwrap();
}
