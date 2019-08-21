#![deny(warnings)]
extern crate hyper;
extern crate pretty_env_logger;

use std::env;
use std::io::{self, Write};

use hyper::Client;

// A simple type alias so as to DRY.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    // Some simple CLI args requirements...
    let url = match env::args().nth(1) {
        Some(url) => url,
        None => {
            println!("Usage: client <url>");
            return Ok(());
        }
    };

    // HTTPS requires picking a TLS implementation, so give a better
    // warning if the user tries to request an 'https' URL.
    let url = url.parse::<hyper::Uri>().unwrap();
    if url.scheme_part().map(|s| s.as_ref()) != Some("http") {
        println!("This example only works with 'http' URLs.");
        return Ok(());
    }

    fetch_url(url).await
}

async fn fetch_url(url: hyper::Uri) -> Result<()> {
    let client = Client::new();

    let res = client.get(url).await?;

    println!("Response: {}", res.status());
    println!("Headers: {:#?}\n", res.headers());

    let mut body = res.into_body();

    while let Some(next) = body.next().await {
        let chunk = next?;
        io::stdout().write_all(&chunk)?;
    }

    println!("\n\nDone!");

    Ok(())
}
