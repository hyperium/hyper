//! HTTP extensions.

use bytes::Bytes;
#[cfg(feature = "http1")]
use http::header::{HeaderName, IntoHeaderName, ValueIter};
use http::HeaderMap;
#[cfg(feature = "http2")]
use std::fmt;

#[cfg(feature = "http2")]
/// Represents the `:protocol` pseudo-header used by
/// the [Extended CONNECT Protocol].
///
/// [Extended CONNECT Protocol]: https://datatracker.ietf.org/doc/html/rfc8441#section-4
#[derive(Clone, Eq, PartialEq)]
pub struct Protocol {
    inner: h2::ext::Protocol,
}

#[cfg(feature = "http2")]
impl Protocol {
    /// Converts a static string to a protocol name.
    pub const fn from_static(value: &'static str) -> Self {
        Self {
            inner: h2::ext::Protocol::from_static(value),
        }
    }

    /// Returns a str representation of the header.
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    pub(crate) fn from_inner(inner: h2::ext::Protocol) -> Self {
        Self { inner }
    }

    pub(crate) fn into_inner(self) -> h2::ext::Protocol {
        self.inner
    }
}

#[cfg(feature = "http2")]
impl<'a> From<&'a str> for Protocol {
    fn from(value: &'a str) -> Self {
        Self {
            inner: h2::ext::Protocol::from(value),
        }
    }
}

#[cfg(feature = "http2")]
impl AsRef<[u8]> for Protocol {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

#[cfg(feature = "http2")]
impl fmt::Debug for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

/// A map from header names to their original casing as received in an HTTP message.
///
/// If an HTTP/1 response `res` is parsed on a connection whose option
/// [`http1_preserve_header_case`] was set to true and the response included
/// the following headers:
///
/// ```ignore
/// x-Bread: Baguette
/// X-BREAD: Pain
/// x-bread: Ficelle
/// ```
///
/// Then `res.extensions().get::<HeaderCaseMap>()` will return a map with:
///
/// ```ignore
/// HeaderCaseMap({
///     "x-bread": ["x-Bread", "X-BREAD", "x-bread"],
/// })
/// ```
///
/// [`http1_preserve_header_case`]: /client/struct.Client.html#method.http1_preserve_header_case
#[derive(Clone, Debug)]
pub(crate) struct HeaderCaseMap(HeaderMap<Bytes>);

#[cfg(feature = "http1")]
impl HeaderCaseMap {
    /// Returns a view of all spellings associated with that header name,
    /// in the order they were found.
    pub(crate) fn get_all<'a>(
        &'a self,
        name: &HeaderName,
    ) -> impl Iterator<Item = impl AsRef<[u8]> + 'a> + 'a {
        self.get_all_internal(name).into_iter()
    }

    /// Returns a view of all spellings associated with that header name,
    /// in the order they were found.
    pub(crate) fn get_all_internal<'a>(&'a self, name: &HeaderName) -> ValueIter<'_, Bytes> {
        self.0.get_all(name).into_iter()
    }

    pub(crate) fn default() -> Self {
        Self(Default::default())
    }

    #[cfg(any(test, feature = "ffi"))]
    pub(crate) fn insert(&mut self, name: HeaderName, orig: Bytes) {
        self.0.insert(name, orig);
    }

    pub(crate) fn append<N>(&mut self, name: N, orig: Bytes)
    where
        N: IntoHeaderName,
    {
        self.0.append(name, orig);
    }
}
