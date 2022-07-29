//! DNS Resolution used by the `HttpConnector`.
//!
//! This module contains:
//!
//! - A [`GaiResolver`](GaiResolver) that is the default resolver for the
//!   `HttpConnector`.
//! - The `Name` type used as an argument to custom resolvers.
//!
//! # Resolvers are `Service`s
//!
//! A resolver is just a
//! `Service<Name, Response = impl Iterator<Item = SocketAddr>>`.
//!
//! A simple resolver that ignores the name and always returns a specific
//! address:
//!
//! ```rust,ignore
//! use std::{convert::Infallible, iter, net::SocketAddr};
//!
//! let resolver = tower::service_fn(|_name| async {
//!     Ok::<_, Infallible>(iter::once(SocketAddr::from(([127, 0, 0, 1], 8080))))
//! });
//! ```
use std::error::Error;
use std::net::{SocketAddr};
use std::str::FromStr;
use std::{fmt, vec};

/// A domain name to resolve into IP addresses.
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Name {
    host: Box<str>,
}

/// A resolver using blocking `getaddrinfo` calls in a threadpool.
#[derive(Clone)]
pub struct GaiResolver {
    _priv: (),
}

/// An iterator of IP addresses returned from `getaddrinfo`.
pub struct GaiAddrs {
    inner: SocketAddrs,
}

impl Name {
    pub(super) fn new(host: Box<str>) -> Name {
        Name { host }
    }

    /// View the hostname as a string slice.
    pub fn as_str(&self) -> &str {
        &self.host
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.host, f)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.host, f)
    }
}

impl FromStr for Name {
    type Err = InvalidNameError;

    fn from_str(host: &str) -> Result<Self, Self::Err> {
        // Possibly add validation later
        Ok(Name::new(host.into()))
    }
}

/// Error indicating a given string was not a valid domain name.
#[derive(Debug)]
pub struct InvalidNameError(());

impl fmt::Display for InvalidNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Not a valid domain name")
    }
}

impl Error for InvalidNameError {}

impl GaiResolver {
    /// Construct a new `GaiResolver`.
    pub fn new() -> Self {
        GaiResolver { _priv: () }
    }
}

impl fmt::Debug for GaiResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("GaiResolver")
    }
}

impl Iterator for GaiAddrs {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl fmt::Debug for GaiAddrs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("GaiAddrs")
    }
}

pub(super) struct SocketAddrs {
    iter: vec::IntoIter<SocketAddr>,
}

impl Iterator for SocketAddrs {
    type Item = SocketAddr;
    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        self.iter.next()
    }
}

/*
/// A resolver using `getaddrinfo` calls via the `tokio_executor::threadpool::blocking` API.
///
/// Unlike the `GaiResolver` this will not spawn dedicated threads, but only works when running on the
/// multi-threaded Tokio runtime.
#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
pub struct TokioThreadpoolGaiResolver(());

/// The future returned by `TokioThreadpoolGaiResolver`.
#[cfg(feature = "runtime")]
#[derive(Debug)]
pub struct TokioThreadpoolGaiFuture {
    name: Name,
}

#[cfg(feature = "runtime")]
impl TokioThreadpoolGaiResolver {
    /// Creates a new DNS resolver that will use tokio threadpool's blocking
    /// feature.
    ///
    /// **Requires** its futures to be run on the threadpool runtime.
    pub fn new() -> Self {
        TokioThreadpoolGaiResolver(())
    }
}

#[cfg(feature = "runtime")]
impl Service<Name> for TokioThreadpoolGaiResolver {
    type Response = GaiAddrs;
    type Error = io::Error;
    type Future = TokioThreadpoolGaiFuture;

    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, name: Name) -> Self::Future {
        TokioThreadpoolGaiFuture { name }
    }
}

#[cfg(feature = "runtime")]
impl Future for TokioThreadpoolGaiFuture {
    type Output = Result<GaiAddrs, io::Error>;

    fn poll(self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match ready!(tokio_executor::threadpool::blocking(|| (
            self.name.as_str(),
            0
        )
            .to_socket_addrs()))
        {
            Ok(Ok(iter)) => Poll::Ready(Ok(GaiAddrs {
                inner: IpAddrs { iter },
            })),
            Ok(Err(e)) => Poll::Ready(Err(e)),
            // a BlockingError, meaning not on a tokio_executor::threadpool :(
            Err(e) => Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e))),
        }
    }
}
*/

mod sealed {
    use super::{Name, SocketAddr};
    use crate::common::{task, Future, Poll};
    use tower_service::Service;

    // "Trait alias" for `Service<Name, Response = Addrs>`
    pub(crate) trait Resolve {
        type Addrs: Iterator<Item = SocketAddr>;
        type Error: Into<Box<dyn std::error::Error + Send + Sync>>;
        type Future: Future<Output = Result<Self::Addrs, Self::Error>>;

        fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>>;
        fn resolve(&mut self, name: Name) -> Self::Future;
    }

    impl<S> Resolve for S
    where
        S: Service<Name>,
        S::Response: Iterator<Item = SocketAddr>,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        type Addrs = S::Response;
        type Error = S::Error;
        type Future = S::Future;

        fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
            Service::poll_ready(self, cx)
        }

        fn resolve(&mut self, name: Name) -> Self::Future {
            Service::call(self, name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_from_str() {
        const DOMAIN: &str = "test.example.com";
        let name = Name::from_str(DOMAIN).expect("Should be a valid domain");
        assert_eq!(name.as_str(), DOMAIN);
        assert_eq!(name.to_string(), DOMAIN);
    }
}
