//! HTTP extensions

use bytes::Bytes;
#[cfg(feature = "http1")]
use http::header::{HeaderName, IntoHeaderName, ValueIter};
use http::HeaderMap;

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
