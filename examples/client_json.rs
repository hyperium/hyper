#![deny(warnings)]
#![warn(rust_2018_idioms)]

#[macro_use]
extern crate serde_derive;

use hyper::Client;
use futures_util::TryStreamExt;

// A simple type alias so as to DRY.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> Result<()> {
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
    // asynchronously concatenate chunks of the body
    let body = res.into_body().try_concat().await?;
    // try to parse as json with serde_json
    let users = serde_json::from_slice(&body)?;

    Ok(users)
}

#[derive(Deserialize, Debug)]
struct User {
    id: i32,
    name: String,
}
