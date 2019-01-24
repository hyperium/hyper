//! Services and MakeServices
//!
//! - A [`Service`](service::Service) is a trait representing an asynchronous
//!   function of a request to a response. It's similar to
//!   `async fn(Request) -> Result<Response, Error>`.
//! - A [`MakeService`](service::MakeService) is a trait creating specific
//!   instances of a `Service`.
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
//! [`service_fn`](service::service_fn) and
//! [`service_fn_ok`](service::service_fn_ok) should be sufficient for most
//! cases.
//!
//! # MakeService
//!
//! Since a `Service` is bound to a single connection, a [`Server`](::Server)
//! needs a way to make them as it accepts connections. This is what a
//! `MakeService` does.
//!
//! Resources that need to be shared by all `Service`s can be put into a
//! `MakeService`, and then passed to individual `Service`s when `make_service`
//! is called.

mod make_service;
mod new_service;
mod service;

pub use self::make_service::{make_service_fn, MakeService, MakeServiceRef};
// NewService is soft-deprecated.
#[doc(hidden)]
pub use self::new_service::NewService;
pub use self::service::{service_fn, service_fn_ok, Service};
