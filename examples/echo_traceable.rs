#![deny(warnings)]

use futures_util::TryStreamExt;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use tracing::instrument::Instrument;
use tracing::{event, info, trace, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::prelude::*;

/// This is our service handler. It receives a Request, routes on its
/// path, and returns a Future of a Response.
async fn echo(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        // Serve some instructions at /
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "Try POSTing data to /echo such as: `curl localhost:3000/echo -XPOST -d 'hello world'`",
        ))),

        // Simply echo the body back to the client.
        (&Method::POST, "/echo") => Ok(Response::new(req.into_body())),

        // Convert to uppercase before sending back to client using a stream.
        (&Method::POST, "/echo/uppercase") => {
            let chunk_stream = req.into_body().map_ok(|chunk| {
                chunk
                    .iter()
                    .map(|byte| byte.to_ascii_uppercase())
                    .collect::<Vec<u8>>()
            });
            Ok(Response::new(Body::wrap_stream(chunk_stream)))
        }

        // Reverse the entire body before sending back to the client.
        //
        // Since we don't know the end yet, we can't simply stream
        // the chunks as they arrive as we did with the above uppercase endpoint.
        // So here we do `.await` on the future, waiting on concatenating the full body,
        // then afterwards the content can be reversed. Only then can we return a `Response`.
        (&Method::POST, "/echo/reversed") => {
            let whole_body = hyper::body::to_bytes(req.into_body()).await?;

            let reversed_body = whole_body.iter().rev().cloned().collect::<Vec<u8>>();
            Ok(Response::new(Body::from(reversed_body)))
        }

        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                    opentelemetry::KeyValue::new("service.name", "echo"),
                    opentelemetry::KeyValue::new("service.namespace", "echo-namespace"),
                ]),
            ))
            .init_async_exporter(opentelemetry::runtime::Tokio)
            .expect("Jaeger Tokio async exporter"),
        opentelemetry::runtime::Tokio,
    )
    .build();

    // Setup Tracer Provider
    let provider = opentelemetry::sdk::trace::TracerProvider::builder()
        .with_span_processor(jaeger_processor)
        .build();

    // Get new Tracer from TracerProvider
    let tracer = opentelemetry::trace::TracerProvider::tracer(&provider, "echo_app");

    // Create a layer with the configured tracer
    let telemetry = tracing_opentelemetry::OpenTelemetryLayer::new(tracer);

    // Use tracing subscriber `Registry`, or any other subscriber that `impl LookupSpan`
    tracing_subscriber::registry()
        .with(telemetry)
        .try_init()
        .expect("Default subscriber");

    // Create a span and enter it, returning a guard....
    let root_span = tracing::span!(tracing::Level::TRACE, "root_span");
    async {
        // Generate a `tracing` "event".
        event!(
            Level::TRACE,
            answer = 42,
            question = "life, the universe, and everything"
        );

        let addr = ([127, 0, 0, 1], 3000).into();
        let service = make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(echo)) });
        let server = Server::bind(&addr).serve(service);

        // Generate a `tracing` "event".
        trace!("Listening on http://{}", addr);

        server.await.expect("Server fault");

        // Generate a `tracing` "event".
        info!("Exiting root_span");
    }
    .instrument(root_span)
    .await;

    Ok(())
}
