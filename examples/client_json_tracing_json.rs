#![deny(warnings)]
#![warn(rust_2018_idioms)]

// Statically compile tracing events and spans - "compile out" tracing code.
//
// Usage:
//
// $ cargo run --features="full tracing/max_level_info" --example client_json_tracing
// ...
// Running `target/debug/examples/client_json_tracing`

// etc.

use hyper::body::Buf;
use hyper::Client;
use hyper::JsonLayer;
use serde::Deserialize;
use tracing::info;
use tracing_subscriber::prelude::*;

// A simple type alias so as to DRY.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;


#[tokio::main]
async fn main() -> Result<()> {
    // Set up `tracing-subscriber` to process tracing data.
    tracing_subscriber::registry().with(JsonLayer).init();

    // Log a `tracing` "event".
    info!(status = true, answer = 42, message = "first event");

    let url = "http://jsonplaceholder.typicode.com/users".parse().unwrap();
    let users = fetch_json(url).await?;
    // print users
    println!("users: {:#?}", users);

    // print the sum of ids
    let sum = users.iter().fold(0, |acc, user| acc + user.id);
    println!("sum of ids: {}", sum);
    Ok(())
}

async fn fetch_json(url: hyper::Uri) -> Result<Vec<User>> {
    let client = Client::new();

    // Fetch the url...
    let res = client.get(url).await?;

    // asynchronously aggregate the chunks of the body
    let body = hyper::body::aggregate(res).await?;

    // try to parse as json with serde_json
    let users = serde_json::from_reader(body.reader())?;

    Ok(users)
}

#[derive(Deserialize, Debug)]
struct User {
    id: i32,
    #[allow(unused)]
    name: String,
}
