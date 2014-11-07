extern crate hyper;

use std::sync::TaskPool;
use std::io::net::ip::Ipv4Addr;

static PHRASE: &'static [u8] = b"Hello World!";

fn hyper_handle(mut incoming: hyper::server::Incoming) {
    let mut pool = TaskPool::new(100, || { proc(_) { } });

    for (_, mut res) in incoming {
        pool.execute(proc(_) {
            let mut res = res.start().unwrap();
            res.write(PHRASE).unwrap();
            res.end().unwrap();
        });
    }
}

fn main() {
    hyper::Server::http(Ipv4Addr(127, 0, 0, 1), 3000).listen(hyper_handle).unwrap();
}

