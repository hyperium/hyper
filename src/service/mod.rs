//! Services and NewServices
//!
//! - A [`Service`](Service) is a trait representing an asynchronous function
//!   of a request to a response. It's similar to
//!   `async fn(Request) -> Result<Response, Error>`.
//! - A [`NewService`](NewService) is a trait creating specific instances of a
//!   `Service`.
//!
//! These types are conceptually similar to those in
//! [tower](https://crates.io/crates/tower), while being specific to hyper.
//!
//! # Service
//!
//! In hyper, especially in the server setting, a `Service` is usually bound
//! to a single connection. It defines how to respond to **all** requests that
//! connection will receive.
//!
//! While it's possible to implement `Service` for a type manually, the helpers
//! [`service_fn`](service_fn) and [`service_fn_ok`](service_fn_ok) should be
//! sufficient for most cases.
//!
//! # NewService
//!
//! Since a `Service` is bound to a single connection, a [`Server`](::Server)
//! needs a way to make them as it accepts connections. This is what a
//! `NewService` does.
//!
//! Resources that need to be shared by all `Service`s can be put into a
//! `NewService`, and then passed to individual `Service`s when `new_service`
//! is called.
mod new_service;
mod service;

pub use self::new_service::{NewService};
pub use self::service::{service_fn, service_fn_ok, Service};
