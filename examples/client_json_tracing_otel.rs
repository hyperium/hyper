#![deny(warnings)]
#![warn(rust_2018_idioms)]

// Statically compile tracing events and spans - "compile out" tracing code.
//
// Usage:
//
// $ cargo run --features="full tracing/max_level_info" --example client_otel_tracing
// ...
// Running `target/debug/examples/client_otel_tracing_off`
// Hyper tracing event:
//   level=Level(Info)
//   target="client_otel_tracing"
//   name="event examples/client_otel_tracing.rs:24"
//   field=status
//   field=answer
//   field=message
// etc.

use hyper::body::Buf;
use hyper::Client;
//use hyper::OtelLayer;
use serde::Deserialize;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::prelude::*;
//use tracing_subscriber::Layer;
//use tracing_subscriber::Registry;

// A simple type alias so as to DRY.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> Result<()> {
    // First create propagators
    let baggage_propagator = opentelemetry::sdk::propagation::BaggagePropagator::new();
    let trace_context_propagator = opentelemetry::sdk::propagation::TraceContextPropagator::new();
    let jaeger_propagator = opentelemetry_jaeger::Propagator::new();
    opentelemetry::global::set_text_map_propagator(
        opentelemetry::sdk::propagation::TraceContextPropagator::new(),
    );

    // Second compose propagators
    let _composite_propagator =  opentelemetry::sdk::propagation::TextMapCompositePropagator::new(vec![
        Box::new(baggage_propagator),
        Box::new(trace_context_propagator),
        Box::new(jaeger_propagator),
    ]);
    // Third create Jaeger pipeline
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("client_json2")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();
    // Initialize `tracing` using `opentelemetry-tracing` and configure stdout logging
    tracing_subscriber::Registry::default()
        .with(tracing_subscriber::EnvFilter::new("TRACE"))
        .with(hyper::OtelLayer::new(tracer))
        .with(tracing_subscriber::fmt::layer())
        //.with(tracing_tree::HierarchicalLayer::new(2))
        .init();

    // Trace executed (async) code
    // tracing::subscriber::with_default(subscriber, || async {
    // Create a span and enter it, returning a guard....
    let root_span = tracing::span!(tracing::Level::INFO, "root_span_echo").entered();
    root_span.in_scope(|| async {
        // Log a `tracing` "event".
        info!(status = true, answer = 42, message = "first event");

        let url = "http://jsonplaceholder.typicode.com/users".parse().unwrap();
        let users = fetch_json(url).await.expect("Vector of user data");
        // print users
        println!("users: {:#?}", users);

        // print the sum of ids
        let sum = users.iter().fold(0, |acc, user| acc + user.id);
        println!("sum of ids: {}", sum);

        // ...but, it can also be exited explicitly, returning the `Span`
        // struct:
        //let _root_span = root_span.exit();
    }).await;
    opentelemetry::global::shutdown_tracer_provider();
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
