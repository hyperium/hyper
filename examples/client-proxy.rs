#![deny(warnings)]
extern crate hyper;

extern crate env_logger;

use std::env;
use std::io;

use hyper::Client;
use hyper::header::Connection;
use hyper::proxy;

// This example assumes that you are running a proxy on port 8080.
// One option is https://mitmproxy.org/
// If you just want to see the output of hyper you can run `nc -l 8080`
fn main() {
    env_logger::init().unwrap();

    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client-proxy <url>");
            return;
        }
    };

    let client = Client::with_proxy_config(proxy::Config {
        proxy_host: "127.0.0.1".to_string(),
        proxy_port: 8080,
        proxy_version: "1.1".to_string(),
        proxy_policy: proxy::ProxyPolicy::ProxyAll,
        proxy_authorization: "1234".to_string()
    });

    let send = client.get(&*url)
        .header(Connection::close())
        .send();
    match send  {
        Ok(mut res) => {
            println!("Response: {}", res.status);
            println!("Headers:\n{}", res.headers);
            io::copy(&mut res, &mut io::stdout()).unwrap();
        },
        Err(e) => {
            println!("Failed {:?}", e);
        }
    }
}
