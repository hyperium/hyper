//! Pieces pertaining to the HTTP message protocol.
use http::{HeaderMap, Method, StatusCode, Uri, Version};

pub(crate) use self::body_length::DecodedLength;
pub(crate) use self::h1::{dispatch, Conn, ServerTransaction};

pub(crate) mod h1;
pub(crate) mod h2;

/// An Incoming Message head. Includes request/status line, and headers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MessageHead<S> {
    /// HTTP version of the message.
    pub version: Version,
    /// Subject (request line or status line) of Incoming message.
    pub subject: S,
    /// Headers of the Incoming message.
    pub headers: HeaderMap,
}

/// An incoming request message.
pub type RequestHead = MessageHead<RequestLine>;

#[derive(Debug, Default, PartialEq)]
pub struct RequestLine(pub Method, pub Uri);

/// An incoming response message.
pub type ResponseHead = MessageHead<StatusCode>;

#[derive(Debug)]
pub enum BodyLength {
    /// Content-Length
    Known(u64),
    /// Transfer-Encoding: chunked (if h1)
    Unknown,
}

/// Status of when a Disaptcher future completes.
pub(crate) enum Dispatched {
    /// Dispatcher completely shutdown connection.
    Shutdown,
    /// Dispatcher has pending upgrade, and so did not shutdown.
    Upgrade(crate::upgrade::Pending),
}

/// A separate module to encapsulate the invariants of the DecodedLength type.
mod body_length {
    use std::fmt;

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) struct DecodedLength(u64);

    const MAX_LEN: u64 = std::u64::MAX - 2;

    impl DecodedLength {
        pub(crate) const CLOSE_DELIMITED: DecodedLength = DecodedLength(::std::u64::MAX);
        pub(crate) const CHUNKED: DecodedLength = DecodedLength(::std::u64::MAX - 1);
        pub(crate) const ZERO: DecodedLength = DecodedLength(0);

        #[cfg(test)]
        pub(crate) fn new(len: u64) -> Self {
            debug_assert!(len <= MAX_LEN);
            DecodedLength(len)
        }

        /// Takes the length as a content-length without other checks.
        ///
        /// Should only be called if previously confirmed this isn't
        /// CLOSE_DELIMITED or CHUNKED.
        #[inline]
        pub(crate) fn danger_len(self) -> u64 {
            debug_assert!(self.0 < Self::CHUNKED.0);
            self.0
        }

        /// Converts to an Option<u64> representing a Known or Unknown length.
        pub(crate) fn into_opt(self) -> Option<u64> {
            match self {
                DecodedLength::CHUNKED | DecodedLength::CLOSE_DELIMITED => None,
                DecodedLength(known) => Some(known),
            }
        }

        /// Checks the `u64` is within the maximum allowed for content-length.
        pub(crate) fn checked_new(len: u64) -> Result<Self, crate::error::Parse> {
            if len <= MAX_LEN {
                Ok(DecodedLength(len))
            } else {
                warn!("content-length bigger than maximum: {} > {}", len, MAX_LEN);
                Err(crate::error::Parse::TooLarge)
            }
        }

        pub(crate) fn sub_if(&mut self, amt: u64) {
            match *self {
                DecodedLength::CHUNKED | DecodedLength::CLOSE_DELIMITED => (),
                DecodedLength(ref mut known) => {
                    *known -= amt;
                }
            }
        }
    }

    impl fmt::Debug for DecodedLength {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match *self {
                DecodedLength::CLOSE_DELIMITED => f.write_str("CLOSE_DELIMITED"),
                DecodedLength::CHUNKED => f.write_str("CHUNKED"),
                DecodedLength(n) => f.debug_tuple("DecodedLength").field(&n).finish(),
            }
        }
    }

    impl fmt::Display for DecodedLength {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match *self {
                DecodedLength::CLOSE_DELIMITED => f.write_str("close-delimited"),
                DecodedLength::CHUNKED => f.write_str("chunked encoding"),
                DecodedLength::ZERO => f.write_str("empty"),
                DecodedLength(n) => write!(f, "content-length ({} bytes)", n),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn sub_if_known() {
            let mut len = DecodedLength::new(30);
            len.sub_if(20);

            assert_eq!(len.0, 10);
        }

        #[test]
        fn sub_if_chunked() {
            let mut len = DecodedLength::CHUNKED;
            len.sub_if(20);

            assert_eq!(len, DecodedLength::CHUNKED);
        }
    }
}
