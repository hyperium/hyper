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
// use hyper::OtelLayer;
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
    // Set up `tracing-subscriber` to process tracing data.
    // Create a jaeger exporter pipeline for a `trace_demo` service.
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 6831));
    // Build a Jaeger batch span processor
    let jaeger_processor = opentelemetry::sdk::trace::BatchSpanProcessor::builder(
        opentelemetry_jaeger::new_pipeline()
            .with_service_name("mre-jaeger")
            .with_agent_endpoint(addr)
            .with_trace_config(opentelemetry::sdk::trace::config().with_resource(
                opentelemetry::sdk::Resource::new(vec![
                    opentelemetry::KeyValue::new("service.name", "client"),
                    opentelemetry::KeyValue::new("service.namespace", "client-namespace"),
                ]),
            ))
            .init_async_exporter(opentelemetry::runtime::Tokio)
            .expect("Jaeger Tokio async exporter"),
        opentelemetry::runtime::Tokio,
    )
    .build();
    // Setup Tracer Provider
    let provider = opentelemetry::sdk::trace::TracerProvider::builder()
        // We can build a span processor and pass it into provider.
        .with_span_processor(jaeger_processor)
        .build();
    // Get new Tracer from TracerProvider
    let tracer = opentelemetry::trace::TracerProvider::tracer(&provider, "client_json", None);
    // Create a layer with the configured tracer
    let telemetry = hyper::OtelLayer::layer().with_tracer(tracer);
    // Use the tracing subscriber `Registry`, or any other subscriber
    // that impls `LookupSpan`
    tracing_subscriber::registry()
        .with(telemetry)
        .try_init()
        .expect("Default subscriber");

    //let subscriber = tracing_subscriber::Registry::default().with(telemetry);

    // Trace executed (async) code
    //tracing::subscriber::with_default(subscriber, || async {
    // Create a span and enter it, returning a guard....
    let root_span = tracing::span!(tracing::Level::INFO, "root_span_echo").entered();

    // We are now inside the span! Like `enter()`, the guard returned by
    // `entered()` will exit the span when it is dropped...

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
    let _root_span = root_span.exit();
    //}).await;
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
