//! Headers container, and common header fields.
//!
//! hyper has the opinion that Headers should be strongly-typed, because that's
//! why we're using Rust in the first place. To set or get any header, an object
//! must implement the `Header` trait from this module. Several common headers
//! are already provided, such as `Host`, `ContentType`, `UserAgent`, and others.
//!
//! ## Mime
//!
//! Several header fields use MIME values for their contents. Keeping with the
//! strongly-typed theme, the [mime](http://seanmonstar.github.io/mime.rs) crate
//! is used, such as `ContentType(pub Mime)`.
use std::any::Any;
use std::ascii::OwnedAsciiExt;
use std::char::is_lowercase;
use std::fmt::{mod, Show};
use std::from_str::{FromStr, from_str};
use std::mem::{transmute, transmute_copy};
use std::raw::TraitObject;
use std::str::{from_utf8, SendStr, Slice};
use std::string::raw;
use std::collections::hashmap::{HashMap, Entries};

use mime::Mime;

use rfc7230::read_header;
use {HttpResult};

/// A trait for any object that will represent a header field and value.
pub trait Header: Any {
    /// Returns the name of the header field this belongs to.
    fn header_name(marker: Option<Self>) -> SendStr;
    /// Parse a header from a raw stream of bytes.
    ///
    /// It's possible that a request can include a header field more than once,
    /// and in that case, the slice will have a length greater than 1. However,
    /// it's not necessarily the case that a Header is *allowed* to have more
    /// than one field value. If that's the case, you **should** return `None`
    /// if `raw.len() > 1`.
    fn parse_header(raw: &[Vec<u8>]) -> Option<Self>;
    /// Format a header to be output into a TcpStream.
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result;
}

/// A trait for downcasting owned TraitObjects to another type without checking
/// `AnyRefExt::is::<T>(t)` first.
trait UncheckedAnyRefExt<'a> {
    /// This will downcast an object to another type without checking that it is
    /// legal. Do not call this unless you are ABSOLUTE SURE of the types.
    unsafe fn downcast_ref_unchecked<T: 'static>(self) -> &'a T;
}

impl<'a> UncheckedAnyRefExt<'a> for &'a Header + 'a {
    #[inline]
    unsafe fn downcast_ref_unchecked<T: 'static>(self) -> &'a T {
        let to: TraitObject = transmute_copy(&self);
        transmute(to.data)
    }
}

fn header_name<T: Header>() -> SendStr {
    let name = Header::header_name(None::<T>);
    debug_assert!(name.as_slice().chars().all(|c| c == '-' || is_lowercase(c)),
        "Header names should be lowercase: {}", name);
    name
}

/// A map of header fields on requests and responses.
pub struct Headers {
    data: HashMap<SendStr, Item>
}

impl Headers {

    /// Creates a new, empty headers map.
    pub fn new() -> Headers {
        Headers {
            data: HashMap::new()
        }
    }

    #[doc(hidden)]
    pub fn from_raw<R: Reader>(rdr: &mut R) -> HttpResult<Headers> {
        let mut headers = Headers::new();
        loop {
            match try!(read_header(rdr)) {
                Some((name, value)) => {
                    // read_header already checks that name is a token, which 
                    // means its safe utf8
                    let name = unsafe {
                        raw::from_utf8(name)
                    }.into_ascii_lower().into_maybe_owned();
                    match headers.data.find_or_insert(name, Raw(vec![])) {
                        &Raw(ref mut pieces) => pieces.push(value),
                        // at this point, Raw is the only thing that has been inserted
                        _ => unreachable!()
                    }
                },
                None => break,
            }
        }
        Ok(headers)
    }

    /// Set a header field to the corresponding value.
    ///
    /// The field is determined by the type of the value being set.
    pub fn set<H: Header + 'static>(&mut self, value: H) {
        self.data.insert(header_name::<H>(), Typed(box value));
    }

    /// Get a clone of the header field's value, if it exists.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::{Headers, ContentType};
    /// # let mut headers = Headers::new();
    /// let content_type = headers.get::<ContentType>();
    /// ```
    pub fn get<H: Header + Clone + 'static>(&mut self) -> Option<H> {
        self.get_ref().map(|v: &H| v.clone())
    }

    /// Get a reference to the header field's value, if it exists.
    pub fn get_ref<H: Header + 'static>(&mut self) -> Option<&H> {
        self.data.find_mut(&header_name::<H>()).and_then(|item| {
            debug!("get_ref, name={}, val={}", header_name::<H>(), item);
            let header = match *item {
                Raw(ref raw) => match Header::parse_header(raw.as_slice()) {
                    Some::<H>(h) => {
                        h
                    },
                    None => return None
                },
                Typed(..) => return Some(item)
            };
            *item = Typed(box header as Box<Header + 'static>);
            Some(item)
        }).and_then(|item| {
            debug!("downcasting {}", item);
            let ret = match *item {
                Typed(ref val) => {
                    unsafe {
                        Some(val.downcast_ref_unchecked())
                    }
                },
                Raw(..) => unreachable!()
            };
            debug!("returning {}", ret.is_some());
            ret
        })
    }

    /// Returns a boolean of whether a certain header is in the map.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::{Headers, ContentType};
    /// # let mut headers = Headers::new();
    /// let has_type = headers.has::<ContentType>();
    /// ```
    pub fn has<H: Header + 'static>(&self) -> bool {
        self.data.contains_key(&header_name::<H>())
    }

    /// Removes a header from the map, if one existed. Returns true if a header has been removed.
    pub fn remove<H: Header>(&mut self) -> bool {
        self.data.remove(&Header::header_name(None::<H>))
    }

    /// Returns an iterator over the header fields.
    pub fn iter<'a>(&'a self) -> HeadersItems<'a> {
        HeadersItems {
            inner: self.data.iter()
        }
    }
}

impl fmt::Show for Headers {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!("Headers {\n".fmt(fmt));
        for (k, v) in self.iter() {
            try!(write!(fmt, "\t{}: {}\n", k, v));
        }
        "}".fmt(fmt)
    }
}

/// An `Iterator` over the fields in a `Headers` map.
pub struct HeadersItems<'a> {
    inner: Entries<'a, SendStr, Item>
}

impl<'a> Iterator<(&'a SendStr, HeaderView<'a>)> for HeadersItems<'a> {
    fn next(&mut self) -> Option<(&'a SendStr, HeaderView<'a>)> {
        match self.inner.next() {
            Some((k, v)) => Some((k, HeaderView(v))),
            None => None
        }
    }
}

/// Returned with the `HeadersItems` iterator.
pub struct HeaderView<'a>(&'a Item);

impl<'a> fmt::Show for HeaderView<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let HeaderView(item) = *self;
        item.fmt(fmt)
    }
}

impl Collection for Headers {
    fn len(&self) -> uint {
        self.data.len()
    }
}

impl Mutable for Headers {
    fn clear(&mut self) {
        self.data.clear()
    }
}

enum Item {
    Raw(Vec<Vec<u8>>),
    Typed(Box<Header + 'static>)
}

impl fmt::Show for Item {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Raw(ref v) => {
                for part in v.iter() {
                    try!(fmt.write(part.as_slice()));
                }
                Ok(())
            },
            Typed(ref h) => h.fmt_header(fmt)
        }
    }
}


// common headers

/// The `Host` header.
///
/// HTTP/1.1 requires that all requests include a `Host` header, and so hyper
/// client requests add one automatically.
///
/// Currently is just a String, but it should probably become a better type,
/// like url::Host or something.
#[deriving(Clone, PartialEq, Show)]
pub struct Host(pub String);

impl Header for Host {
    fn header_name(_: Option<Host>) -> SendStr {
        Slice("host")
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Host> {
        from_one_raw_str(raw).map(|s| Host(s))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Host(ref value) = *self;
        value.fmt(fmt)
    }
}

/// The `Content-Length` header.
///
/// Simply a wrapper around a `uint`.
#[deriving(Clone, PartialEq, Show)]
pub struct ContentLength(pub uint);

impl Header for ContentLength {
    fn header_name(_: Option<ContentLength>) -> SendStr {
        Slice("content-length")
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<ContentLength> {
        from_one_raw_str(raw).map(|u| ContentLength(u))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let ContentLength(ref value) = *self;
        value.fmt(fmt)
    }
}

/// The `Content-Type` header.
///
/// Used to describe the MIME type of message body. Can be used with both
/// requests and responses.
#[deriving(Clone, PartialEq, Show)]
pub struct ContentType(pub Mime);

impl Header for ContentType {
    fn header_name(_: Option<ContentType>) -> SendStr {
        Slice("content-type")
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<ContentType> {
        from_one_raw_str(raw).map(|mime| ContentType(mime))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let ContentType(ref value) = *self;
        value.fmt(fmt)
    }
}

/// The `Accept` header.
///
/// The `Accept` header is used to tell a server which content-types the client
/// is capable of using. It can be a comma-separated list of `Mime`s, and the
/// priority can be indicated with a `q` parameter.
///
/// Example:
///
/// ```
/// # use hyper::header::{Headers, Accept};
/// use hyper::mime::{Mime, Text, Html, Xml};
/// # let mut headers = Headers::new();
/// headers.set(Accept(vec![ Mime(Text, Html, vec![]), Mime(Text, Xml, vec![]) ]));
/// ```
#[deriving(Clone, PartialEq, Show)]
pub struct Accept(pub Vec<Mime>);

impl Header for Accept {
    fn header_name(_: Option<Accept>) -> SendStr {
        Slice("accept")
    }

    fn parse_header(_raw: &[Vec<u8>]) -> Option<Accept> {
        unimplemented!()
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let Accept(ref value) = *self;
        let last = value.len() - 1;
        for (i, mime) in value.iter().enumerate() {
            try!(mime.fmt(fmt));
            if i < last {
                try!(", ".fmt(fmt));
            }
        }
        Ok(())
    }
}

/// The `Connection` header.
///
/// Describes whether the socket connection should be closed or reused after
/// this request/response is completed.
#[deriving(Clone, PartialEq, Show)]
pub enum Connection {
    /// The `keep-alive` connection value.
    KeepAlive,
    /// The `close` connection value.
    Close
}

impl FromStr for Connection {
    fn from_str(s: &str) -> Option<Connection> {
        debug!("Connection::from_str =? {}", s);
        match s {
            "keep-alive" => Some(KeepAlive),
            "close" => Some(Close),
            _ => None
        }
    }
}

impl Header for Connection {
    fn header_name(_: Option<Connection>) -> SendStr {
        Slice("connection")
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Connection> {
        from_one_raw_str(raw)
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            KeepAlive => "keep-alive",
            Close => "close",
        }.fmt(fmt)
    }
}

/// The `Transfer-Encoding` header.
///
/// This header describes the encoding of the message body. It can be
/// comma-separated, including multiple encodings.
///
/// ```notrust
/// Transfer-Encoding: gzip, chunked
/// ```
///
/// According to the spec, if a `Content-Length` header is not included,
/// this header should include `chunked` as the last encoding.
///
/// The implementation uses a vector of `Encoding` values.
#[deriving(Clone, PartialEq, Show)]
pub struct TransferEncoding(pub Vec<Encoding>);

/// A value to be used with the `Transfer-Encoding` header.
///
/// Example:
///
/// ```
/// # use hyper::header::{Headers, TransferEncoding, Gzip, Chunked};
/// # let mut headers = Headers::new();
/// headers.set(TransferEncoding(vec![Gzip, Chunked]));
#[deriving(Clone, PartialEq, Show)]
pub enum Encoding {
    /// The `chunked` encoding.
    Chunked,

    // TODO: #2 implement this in `HttpReader`.
    /// The `gzip` encoding.
    Gzip,
    /// The `deflate` encoding.
    Deflate,
    /// The `compress` encoding.
    Compress,
    /// Some other encoding that is less common, can be any String.
    EncodingExt(String)
}

impl FromStr for Encoding {
    fn from_str(s: &str) -> Option<Encoding> {
        match s {
            "chunked" => Some(Chunked),
            _ => None
        }
    }
}

impl Header for TransferEncoding {
    fn header_name(_: Option<TransferEncoding>) -> SendStr {
        Slice("transfer-encoding")
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<TransferEncoding> {
        if raw.len() != 1 {
            return None;
        }
        // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
        match from_utf8(unsafe { raw.as_slice().unsafe_get(0).as_slice() }) {
            Some(s) => {
                Some(TransferEncoding(s.as_slice()
                     .split([',', ' '].as_slice())
                     .filter_map(from_str)
                     .collect()))
            }
            None => None
        }
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let TransferEncoding(ref parts) = *self;
        let last = parts.len() - 1;
        for (i, part) in parts.iter().enumerate() {
            try!(part.fmt(fmt));
            if i < last {
                try!(", ".fmt(fmt));
            }
        }
        Ok(())
    }
}

/// The `User-Agent` header field.
///
/// They can contain any value, so it just wraps a `String`.
#[deriving(Clone, PartialEq, Show)]
pub struct UserAgent(pub String);

impl Header for UserAgent {
    fn header_name(_: Option<UserAgent>) -> SendStr {
        Slice("user-agent")
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<UserAgent> {
        from_one_raw_str(raw).map(|s| UserAgent(s))
    }

    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let UserAgent(ref value) = *self;
        value.fmt(fmt)
    }
}

fn from_one_raw_str<T: FromStr>(raw: &[Vec<u8>]) -> Option<T> {
    if raw.len() != 1 {
        return None;
    }
    // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
    match from_utf8(unsafe { raw.as_slice().unsafe_get(0).as_slice() }) {
        Some(s) => FromStr::from_str(s),
        None => None
    }
}

#[cfg(test)]
mod tests {
    use std::io::MemReader;
    use mime::{Mime, Text, Plain};
    use super::{Headers, Header, ContentLength, ContentType};

    fn mem(s: &str) -> MemReader {
        MemReader::new(s.as_bytes().to_vec())
    }

    #[test]
    fn test_from_raw() {
        let mut headers = Headers::from_raw(&mut mem("Content-Length: 10\r\n\r\n")).unwrap();
        assert_eq!(headers.get_ref(), Some(&ContentLength(10)));
    }

    #[test]
    fn test_content_type() {
        let content_type = Header::parse_header(["text/plain".as_bytes().to_vec()].as_slice());
        assert_eq!(content_type, Some(ContentType(Mime(Text, Plain, vec![]))));
    }
}
