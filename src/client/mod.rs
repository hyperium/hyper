//! HTTP Client
use url::Url;

use method::{Get, Head, Post, Delete, Method};

pub use self::request::Request;
pub use self::response::Response;
use {HttpResult};

pub mod request;
pub mod response;


/// Create a GET client request.
pub fn get(url: Url) -> HttpResult<Request> {
    request(Get, url)
}

/// Create a HEAD client request.
pub fn head(url: Url) -> HttpResult<Request> {
    request(Head, url)
}

/// Create a POST client request.
pub fn post(url: Url) -> HttpResult<Request> {
    // TODO: should this accept a Body parameter? or just let user `write` to the request?
    request(Post, url)
}

/// Create a DELETE client request.
pub fn delete(url: Url) -> HttpResult<Request> {
    request(Delete, url)
}

/// Create a client request.
pub fn request(method: Method, url: Url) -> HttpResult<Request> {
    Request::new(method, url)
}
