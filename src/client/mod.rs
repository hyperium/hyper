//! HTTP Client
use url::Url;

use method::{Get, Head, Post, Delete, Method};

pub use self::request::Request;
pub use self::response::Response;
pub use net::{Fresh, Streaming};
use {HttpResult};

pub mod request;
pub mod response;

/// Create a GET client request.
pub fn get(url: Url) -> HttpResult<Request<Fresh>> {
    request(Get, url)
}

/// Create a HEAD client request.
pub fn head(url: Url) -> HttpResult<Request<Fresh>> {
    request(Head, url)
}

/// Create a POST client request.
pub fn post(url: Url) -> HttpResult<Request<Fresh>> {
    // TODO: should this accept a Body parameter? or just let user `write` to the request?
    request(Post, url)
}

/// Create a DELETE client request.
pub fn delete(url: Url) -> HttpResult<Request<Fresh>> {
    request(Delete, url)
}

/// Create a client request.
pub fn request(method: Method, url: Url) -> HttpResult<Request<Fresh>> {
    Request::new(method, url)
}
