#![deny(warnings)]
#![warn(rust_2018_idioms)]

// Statically compile out all tracing and logging events and spans
// - that is, "compile out" all tracing and logging code.
//
// Usage:
//
// $ cargo run --features="full tracing/max_level_off log/max_level_off" --example client_json_tracing_off
// ...
// Running `target/debug/examples/client_json_tracing_off`
// users: [
//     User {
//         id: 1,
//         name: "Leanne Graham",
//     },
//     User {
//         id: 2,
//         name: "Ervin Howell",
// etc.

use hyper::body::Buf;
use hyper::Client;
use hyper::PrintLayer;
// use hyper::JsonLayer;
use serde::Deserialize;
use tracing::info;
use tracing_subscriber::prelude::*;

// A simple type alias so as to DRY.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;


#[tokio::main]
async fn main() -> Result<()> {
    // Set up `tracing-subscriber` to process tracing data.
    // Note:
    // We silence tracing via compile/build time features - see `Cargo.toml`.
    // Hence, no change is required from the `client_json_tracing` example.
    tracing_subscriber::registry().with(PrintLayer).init();

    // "Log" a `tracing` "event".
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
