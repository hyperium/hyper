//! HTTP Tracing
//!
//! Hyper uses the [`tracing`] crate to provide structured, event-based
//! diagnostic information..
//!
//! Span configuration should conform to the OpenTelemetry [Exceptions], [HTTP]
//! and [General] semantic conventions.
//!
//! # Examples
//!
//! For complete working code, take a look at these examples:
//!
//! - [`client_json`]: Hyper client example with no tracing related code.
//! - [`client_json_tracing_otel`]: Hyper client example with OpenTelemetry
//!   and Jaeger tracing.
//! - [`client_json_tracing_off`]: The previous example, now with `Cargo.toml`
//!   settings to statically compile all tracing overhead out of the final
//!   binary.  The final result should be smaller, and faster, than the
//!   [`client_json`] example.
//!
//! ```bash
//! podman run -p6831:6831/udp -p6832:6832/udp -p16686:16686 jaegertracing/all-in-one:latest
//! cargo run --features="full tracing/max_level_trace" --example client_json_tracing_otel
//! firefox http://localhost:16686
//! # See also stdout
//! ```
//!
//! [tracing]: https://docs.rs/tracing
//! [Exceptions]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/semantic_conventions/exceptions.md
//! [General]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/semantic_conventions/span-general.md
//! [HTTP]: https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/trace/semantic_conventions/http.md
//! [client_json]: https://github.com/hyperium/hyper/blob/master/examples/client_json.rs
//! [client_json_tracing_otel]: https://github.com/hyperium/hyper/blob/master/examples/client_json_tracing_otel.rs
//! [client_json_tracing_off]: https://github.com/hyperium/hyper/blob/master/examples/client_json_tracing_off.rs
pub mod values;
pub mod json;
pub mod otel;
pub mod print;
