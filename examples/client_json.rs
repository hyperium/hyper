#![deny(warnings)]
extern crate hyper;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use hyper::Client;
use hyper::rt::{self, Future, Stream};

fn main() {
    let url = "http://jsonplaceholder.typicode.com/users".parse().unwrap();

    // Run the runtime with the future trying to fetch, parse and print json.
    //
    // Note that in more complicated use cases, the runtime should probably
    // run on its own, and futures should just be spawned into it.
    rt::run(fetch_json(url));
}

fn fetch_json(url: hyper::Uri) -> impl Future<Item=(), Error=()> {
    let client = Client::new();

    client
        // Fetch the url...
        .get(url)
        // And then, if we get a response back...
        .and_then(|res| {
            // asynchronously concatenate chunks of the body
            res.into_body().concat2()
        })
        // use the body after concatenation
        .map(|body| {
            // try to parse as json with serde_json
            let users: Vec<User> = serde_json::from_slice(&body).expect("parse json");

            // pretty print result
            println!("{:#?}", users);
        })
        // If there was an error, let the user know...
        .map_err(|err| {
            eprintln!("Error {}", err);
        })
}

#[derive(Deserialize, Debug)]
struct User {
    id: i32,
    name: String,
}