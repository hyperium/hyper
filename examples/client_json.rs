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

    let fut = fetch_json(url)
        // use the parsed vector
        .map(|users| {
            // print users
            println!("users: {:#?}", users);

            // print the sum of ids
            let sum = users.iter().fold(0, |acc, user| acc + user.id);
            println!("sum of ids: {}", sum);
        })
        // if there was an error print it
        .map_err(|e| {
            match e {
                FetchError::Http(e) => eprintln!("http error: {}", e),
                FetchError::Json(e) => eprintln!("json parsing error: {}", e),
            }
        });

    // Run the runtime with the future trying to fetch, parse and print json.
    //
    // Note that in more complicated use cases, the runtime should probably
    // run on its own, and futures should just be spawned into it.
    rt::run(fut);
}

fn fetch_json(url: hyper::Uri) -> impl Future<Item=Vec<User>, Error=FetchError> {
    let client = Client::new();

    client
        // Fetch the url...
        .get(url)
        // And then, if we get a response back...
        .and_then(|res| {
            // asynchronously concatenate chunks of the body
            res.into_body().concat2()
        })
        .from_err::<FetchError>()
        // use the body after concatenation
        .and_then(|body| {
            // try to parse as json with serde_json
            let users = serde_json::from_slice(&body)?;

            Ok(users)
        })
        .from_err()
}

#[derive(Deserialize, Debug)]
struct User {
    id: i32,
    name: String,
}

// Define a type so we can return multiple types of errors
enum FetchError {
    Http(hyper::Error),
    Json(serde_json::Error),
}

impl From<hyper::Error> for FetchError {
    fn from(err: hyper::Error) -> FetchError {
        FetchError::Http(err)
    }
}

impl From<serde_json::Error> for FetchError {
    fn from(err: serde_json::Error) -> FetchError {
        FetchError::Json(err)
    }
}
