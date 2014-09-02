//! # Client
use url::Url;

use method::{Get, Method};

pub use self::request::Request;
pub use self::response::Response;
use {HttpResult};

pub mod request;
pub mod response;


/// Create a GET client request.
pub fn get(url: Url) -> HttpResult<Request> {
    request(Get, url)
}

/// Create a client request.
pub fn request(method: Method, url: Url) -> HttpResult<Request> {
    Request::new(method, url)
}
