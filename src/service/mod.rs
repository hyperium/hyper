//! Asynchronous Services.
//!
//! A [`Service`] is a trait representing an asynchronous
//! function of a request to a response. It's similar to
//! `async fn(Request) -> Result<Response, Error>`.
//!
//! The argument and return value isn't strictly required to be for HTTP.
//! Therefore, hyper uses several "trait aliases" to reduce clutter around
//! bounds. These are:
//!
//! - `HttpService`: This is blanketly implemented for all types that
//!   implement `Service<http::Request<B1>, Response = http::Response<B2>>`.
//!
//! # `HttpService`
//!
//! In hyper, especially in the server setting, a `Service` is usually bound
//! to a single connection. It defines how to respond to **all** requests that
//! connection will receive.
//!
//! The helper [`service_fn`] should be sufficient for most cases, but
//! if you need to implement `Service` for a type manually, you can follow the example
//! in `service_struct_impl.rs`.
//!
//! # Middleware
//!
//! This trait is not [`tower::Service`][tower], so a tower stack has to be wrapped in
//! [`hyper_util::service::TowerToHyperService`][t2h] before a server can be given it.
//! The `tower_layers.rs` example shows the shape, and the [middleware guide][guide]
//! goes further.
//!
//! [tower]: https://docs.rs/tower/latest/tower/trait.Service.html
//! [t2h]: https://docs.rs/hyper-util/latest/hyper_util/service/struct.TowerToHyperService.html
//! [guide]: https://hyper.rs/guides/1/server/middleware/

mod http;
mod service;
mod util;

pub use self::http::HttpService;
pub use self::service::Service;
pub use self::util::service_fn;
