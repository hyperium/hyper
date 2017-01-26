//! Headers container, and common header fields.
//!
//! hyper has the opinion that Headers should be strongly-typed, because that's
//! why we're using Rust in the first place. To set or get any header, an object
//! must implement the `Header` trait from this module. Several common headers
//! are already provided, such as `Host`, `ContentType`, `UserAgent`, and others.
//!
//! # Why Typed?
//!
//! Or, why not stringly-typed? Types give the following advantages:
//!
//! - More difficult to typo, since typos in types should be caught by the compiler
//! - Parsing to a proper type by default
//!
//! # Defining Custom Headers
//!
//! Hyper provides many of the most commonly used headers in HTTP. If
//! you need to define a custom header, it's easy to do while still taking
//! advantage of the type system. Hyper includes a `header!` macro for defining
//! many wrapper-style headers.
//!
//! ```
//! #[macro_use] extern crate hyper;
//! use hyper::header::Headers;
//! header! { (XRequestGuid, "X-Request-Guid") => [String] }
//!
//! fn main () {
//!     let mut headers = Headers::new();
//!
//!     headers.set(XRequestGuid("a proper guid".to_owned()))
//! }
//! ```
//!
//! This works well for simple "string" headers. But the header system
//! actually involves 2 parts: parsing, and formatting. If you need to
//! customize either part, you can do so.
//!
//! ## `Header` and `HeaderFormat`
//!
//! Consider a Do Not Track header. It can be true or false, but it represents
//! that via the numerals `1` and `0`.
//!
//! ```
//! use std::fmt;
//! use hyper::header::{Header, HeaderFormat};
//!
//! #[derive(Debug, Clone, Copy)]
//! struct Dnt(bool);
//!
//! impl Header for Dnt {
//!     fn header_name() -> &'static str {
//!         "DNT"
//!     }
//!
//!     fn parse_header(raw: &[Vec<u8>]) -> hyper::Result<Dnt> {
//!         if raw.len() == 1 {
//!             let line = &raw[0];
//!             if line.len() == 1 {
//!                 let byte = line[0];
//!                 match byte {
//!                     b'0' => return Ok(Dnt(true)),
//!                     b'1' => return Ok(Dnt(false)),
//!                     _ => ()
//!                 }
//!             }
//!         }
//!         Err(hyper::Error::Header)
//!     }
//! }
//!
//! impl HeaderFormat for Dnt {
//!     fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
//!         if self.0 {
//!             f.write_str("1")
//!         } else {
//!             f.write_str("0")
//!         }
//!     }
//! }
//! ```
use std::any::Any;
use std::borrow::{Cow, ToOwned};
//use std::collections::HashMap;
//use std::collections::hash_map::{Iter, Entry};
use std::iter::{FromIterator, IntoIterator};
use std::ops::{Deref, DerefMut};
use std::{mem, fmt};

use {httparse, traitobject};
use typeable::Typeable;
use unicase::UniCase;

use self::internals::{Item, VecMap, Entry};

pub use self::shared::*;
pub use self::common::*;

mod common;
mod internals;
mod shared;
pub mod parsing;

type HeaderName = UniCase<CowStr>;

/// A trait for any object that will represent a header field and value.
///
/// This trait represents the construction and identification of headers,
/// and contains trait-object unsafe methods.
pub trait Header: Clone + Any + Send + Sync {
    /// Returns the name of the header field this belongs to.
    ///
    /// This will become an associated constant once available.
    fn header_name() -> &'static str;
    /// Parse a header from a raw stream of bytes.
    ///
    /// It's possible that a request can include a header field more than once,
    /// and in that case, the slice will have a length greater than 1. However,
    /// it's not necessarily the case that a Header is *allowed* to have more
    /// than one field value. If that's the case, you **should** return `None`
    /// if `raw.len() > 1`.
    fn parse_header(raw: &[Vec<u8>]) -> ::Result<Self>;

}

/// A trait for any object that will represent a header field and value.
///
/// This trait represents the formatting of a `Header` for output to a TcpStream.
pub trait HeaderFormat: fmt::Debug + HeaderClone + Any + Typeable + Send + Sync {
    /// Format a header to be output into a TcpStream.
    ///
    /// This method is not allowed to introduce an Err not produced
    /// by the passed-in Formatter.
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result;

    /// Formats a header over multiple lines.
    ///
    /// The main example here is `Set-Cookie`, which requires that every
    /// cookie being set be specified in a separate line.
    ///
    /// The API here is still being explored, so this is hidden by default.
    /// The passed in formatter doesn't have any public methods, so it would
    /// be quite difficult to depend on this externally.
    #[doc(hidden)]
    #[inline]
    fn fmt_multi_header(&self, f: &mut MultilineFormatter) -> fmt::Result {
        f.fmt_line(&FmtHeader(self))
    }
}

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct MultilineFormatter<'a, 'b: 'a>(Multi<'a, 'b>);

enum Multi<'a, 'b: 'a> {
    Line(&'a str, &'a mut fmt::Formatter<'b>),
    Join(bool, &'a mut fmt::Formatter<'b>),
}

impl<'a, 'b> MultilineFormatter<'a, 'b> {
    fn fmt_line(&mut self, line: &fmt::Display) -> fmt::Result {
        use std::fmt::Write;
        match self.0 {
            Multi::Line(ref name, ref mut f) => {
                try!(f.write_str(*name));
                try!(f.write_str(": "));
                try!(write!(NewlineReplacer(*f), "{}", line));
                f.write_str("\r\n")
            },
            Multi::Join(ref mut first, ref mut f) => {
                if !*first {
                    try!(f.write_str(", "));
                } else {
                    *first = false;
                }
                write!(NewlineReplacer(*f), "{}", line)
            }
        }
    }
}

// Internal helper to wrap fmt_header into a fmt::Display
struct FmtHeader<'a, H: ?Sized + 'a>(&'a H);

impl<'a, H: HeaderFormat + ?Sized + 'a> fmt::Display for FmtHeader<'a, H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt_header(f)
    }
}

struct ValueString<'a>(&'a Item);

impl<'a> fmt::Display for ValueString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.write_h1(&mut MultilineFormatter(Multi::Join(true, f)))
    }
}

struct NewlineReplacer<'a, 'b: 'a>(&'a mut fmt::Formatter<'b>);

impl<'a, 'b> fmt::Write for NewlineReplacer<'a, 'b> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut since = 0;
        for (i, &byte) in s.as_bytes().iter().enumerate() {
            if byte == b'\r' || byte == b'\n' {
                try!(self.0.write_str(&s[since..i]));
                try!(self.0.write_str(" "));
                since = i + 1;
            }
        }
        if since < s.len() {
            self.0.write_str(&s[since..])
        } else {
            Ok(())
        }
    }
}

#[doc(hidden)]
pub trait HeaderClone {
    fn clone_box(&self) -> Box<HeaderFormat + Send + Sync>;
}

impl<T: HeaderFormat + Clone> HeaderClone for T {
    #[inline]
    fn clone_box(&self) -> Box<HeaderFormat + Send + Sync> {
        Box::new(self.clone())
    }
}

impl HeaderFormat + Send + Sync {
    #[inline]
    unsafe fn downcast_ref_unchecked<T: 'static>(&self) -> &T {
        mem::transmute(traitobject::data(self))
    }

    #[inline]
    unsafe fn downcast_mut_unchecked<T: 'static>(&mut self) -> &mut T {
        mem::transmute(traitobject::data_mut(self))
    }
}

impl Clone for Box<HeaderFormat + Send + Sync> {
    #[inline]
    fn clone(&self) -> Box<HeaderFormat + Send + Sync> {
        self.clone_box()
    }
}

#[inline]
fn header_name<T: Header>() -> &'static str {
    <T as Header>::header_name()
}

/// A map of header fields on requests and responses.
#[derive(Clone)]
pub struct Headers {
    //data: HashMap<HeaderName, Item>
    data: VecMap<HeaderName, Item>,
}

impl Headers {

    /// Creates a new, empty headers map.
    pub fn new() -> Headers {
        Headers {
            data: VecMap::new()
        }
    }

    #[doc(hidden)]
    pub fn from_raw(raw: &[httparse::Header]) -> ::Result<Headers> {
        let mut headers = Headers::new();
        for header in raw {
            trace!("raw header: {:?}={:?}", header.name, &header.value[..]);
            let name = UniCase(CowStr(Cow::Owned(header.name.to_owned())));
            let mut item = match headers.data.entry(name) {
                Entry::Vacant(entry) => entry.insert(Item::new_raw(vec![])),
                Entry::Occupied(entry) => entry.into_mut()
            };
            let trim = header.value.iter().rev().take_while(|&&x| x == b' ').count();
            let value = &header.value[.. header.value.len() - trim];
            item.raw_mut().push(value.to_vec());
        }
        Ok(headers)
    }

    /// Set a header field to the corresponding value.
    ///
    /// The field is determined by the type of the value being set.
    pub fn set<H: Header + HeaderFormat>(&mut self, value: H) {
        trace!("Headers.set( {:?}, {:?} )", header_name::<H>(), value);
        self.data.insert(UniCase(CowStr(Cow::Borrowed(header_name::<H>()))),
                         Item::new_typed(Box::new(value)));
    }

    /// Access the raw value of a header.
    ///
    /// Prefer to use the typed getters instead.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # let mut headers = Headers::new();
    /// let raw_content_type = headers.get_raw("content-type");
    /// ```
    pub fn get_raw(&self, name: &str) -> Option<&[Vec<u8>]> {
        self.data
            .get(&UniCase(CowStr(Cow::Borrowed(unsafe { mem::transmute::<&str, &str>(name) }))))
            .map(Item::raw)
    }

    /// Set the raw value of a header, bypassing any typed headers.
    ///
    /// Note: This will completely replace any current value for this
    /// header name.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # let mut headers = Headers::new();
    /// headers.set_raw("content-length", vec![b"5".to_vec()]);
    /// ```
    pub fn set_raw<K: Into<Cow<'static, str>>>(&mut self, name: K,
            value: Vec<Vec<u8>>) {
        let name = name.into();
        trace!("Headers.set_raw( {:?}, {:?} )", name, value);
        self.data.insert(UniCase(CowStr(name)), Item::new_raw(value));
    }

    /// Append a value to raw value of this header.
    ///
    /// If a header already contains a value, this will add another line to it.
    ///
    /// If a header doesnot exist for this name, a new one will be created with
    /// the value.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # let mut headers = Headers::new();
    /// headers.append_raw("x-foo", b"bar".to_vec());
    /// headers.append_raw("x-foo", b"quux".to_vec());
    /// ```
    pub fn append_raw<K: Into<Cow<'static, str>>>(&mut self, name: K, value: Vec<u8>) {
        let name = name.into();
        trace!("Headers.append_raw( {:?}, {:?} )", name, value);
        let name = UniCase(CowStr(name));
        if let Some(item) = self.data.get_mut(&name) {
            item.raw_mut().push(value);
            return;
        }
        self.data.insert(name, Item::new_raw(vec![value]));
    }

    /// Remove a header set by set_raw
    pub fn remove_raw(&mut self, name: &str) {
        trace!("Headers.remove_raw( {:?} )", name);
        self.data.remove(
            &UniCase(CowStr(Cow::Borrowed(unsafe { mem::transmute::<&str, &str>(name) })))
        );
    }

    /// Get a reference to the header field's value, if it exists.
    pub fn get<H: Header + HeaderFormat>(&self) -> Option<&H> {
        self.data.get(&UniCase(CowStr(Cow::Borrowed(header_name::<H>()))))
        .and_then(Item::typed::<H>)
    }

    /// Get a mutable reference to the header field's value, if it exists.
    pub fn get_mut<H: Header + HeaderFormat>(&mut self) -> Option<&mut H> {
        self.data.get_mut(&UniCase(CowStr(Cow::Borrowed(header_name::<H>()))))
        .and_then(Item::typed_mut::<H>)
    }

    /// Returns a boolean of whether a certain header is in the map.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # use hyper::header::ContentType;
    /// # let mut headers = Headers::new();
    /// let has_type = headers.has::<ContentType>();
    /// ```
    pub fn has<H: Header + HeaderFormat>(&self) -> bool {
        self.data.contains_key(&UniCase(CowStr(Cow::Borrowed(header_name::<H>()))))
    }

    /// Removes a header from the map, if one existed.
    /// Returns true if a header has been removed.
    pub fn remove<H: Header + HeaderFormat>(&mut self) -> bool {
        trace!("Headers.remove( {:?} )", header_name::<H>());
        self.data.remove(&UniCase(CowStr(Cow::Borrowed(header_name::<H>())))).is_some()
    }

    /// Returns an iterator over the header fields.
    pub fn iter(&self) -> HeadersItems {
        HeadersItems {
            inner: self.data.iter()
        }
    }

    /// Returns the number of headers in the map.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Remove all headers from the map.
    pub fn clear(&mut self) {
        self.data.clear()
    }
}

impl PartialEq for Headers {
    fn eq(&self, other: &Headers) -> bool {
        if self.len() != other.len() {
            return false;
        }

        for header in self.iter() {
            match other.get_raw(header.name()) {
                Some(val) if val == self.get_raw(header.name()).unwrap() => {},
                _ => { return false; }
            }
        }
        true
    }
}

impl fmt::Display for Headers {
   fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for header in self.iter() {
            try!(fmt::Display::fmt(&header, f));
        }
        Ok(())
    }
}

impl fmt::Debug for Headers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("Headers { "));
        for header in self.iter() {
            try!(write!(f, "{:?}, ", header));
        }
        try!(f.write_str("}"));
        Ok(())
    }
}

/// An `Iterator` over the fields in a `Headers` map.
pub struct HeadersItems<'a> {
    inner: ::std::slice::Iter<'a, (HeaderName, Item)>
}

impl<'a> Iterator for HeadersItems<'a> {
    type Item = HeaderView<'a>;

    fn next(&mut self) -> Option<HeaderView<'a>> {
        self.inner.next().map(|&(ref k, ref v)| HeaderView(k, v))
    }
}

/// Returned with the `HeadersItems` iterator.
pub struct HeaderView<'a>(&'a HeaderName, &'a Item);

impl<'a> HeaderView<'a> {
    /// Check if a HeaderView is a certain Header.
    #[inline]
    pub fn is<H: Header>(&self) -> bool {
        UniCase(CowStr(Cow::Borrowed(header_name::<H>()))) == *self.0
    }

    /// Get the Header name as a slice.
    #[inline]
    pub fn name(&self) -> &'a str {
        self.0.as_ref()
    }

    /// Cast the value to a certain Header type.
    #[inline]
    pub fn value<H: Header + HeaderFormat>(&self) -> Option<&'a H> {
        self.1.typed::<H>()
    }

    /// Get just the header value as a String.
    ///
    /// This will join multiple values of this header with a `, `.
    ///
    /// **Warning:** This may not be the format that should be used to send
    /// a Request or Response.
    #[inline]
    pub fn value_string(&self) -> String {
        ValueString(self.1).to_string()
    }
}

impl<'a> fmt::Display for HeaderView<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.1.write_h1(&mut MultilineFormatter(Multi::Line(&self.0, f)))
    }
}

impl<'a> fmt::Debug for HeaderView<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<'a> Extend<HeaderView<'a>> for Headers {
    fn extend<I: IntoIterator<Item=HeaderView<'a>>>(&mut self, iter: I) {
        for header in iter {
            self.data.insert((*header.0).clone(), (*header.1).clone());
        }
    }
}

impl<'a> FromIterator<HeaderView<'a>> for Headers {
    fn from_iter<I: IntoIterator<Item=HeaderView<'a>>>(iter: I) -> Headers {
        let mut headers = Headers::new();
        headers.extend(iter);
        headers
    }
}

deprecated! {
    #[deprecated(note="The semantics of formatting a HeaderFormat directly are not clear")]
    impl<'a> fmt::Display for &'a (HeaderFormat + Send + Sync) {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let mut multi = MultilineFormatter(Multi::Join(true, f));
            self.fmt_multi_header(&mut multi)
        }
    }
}

/// A wrapper around any Header with a Display impl that calls fmt_header.
///
/// This can be used like so: `format!("{}", HeaderFormatter(&header))` to
/// get the 'value string' representation of this Header.
///
/// Note: This may not necessarily be the value written to stream, such
/// as with the SetCookie header.
deprecated! {
    #[deprecated(note="The semantics of formatting a HeaderFormat directly are not clear")]
    pub struct HeaderFormatter<'a, H: HeaderFormat>(pub &'a H);
}

#[allow(deprecated)]
impl<'a, H: HeaderFormat> fmt::Display for HeaderFormatter<'a, H> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut multi = MultilineFormatter(Multi::Join(true, f));
        self.0.fmt_multi_header(&mut multi)
    }
}

#[allow(deprecated)]
impl<'a, H: HeaderFormat> fmt::Debug for HeaderFormatter<'a, H> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
struct CowStr(Cow<'static, str>);

impl Deref for CowStr {
    type Target = Cow<'static, str>;

    fn deref(&self) -> &Cow<'static, str> {
        &self.0
    }
}

impl fmt::Debug for CowStr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for CowStr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl DerefMut for CowStr {
    fn deref_mut(&mut self) -> &mut Cow<'static, str> {
        &mut self.0
    }
}

impl AsRef<str> for CowStr {
    fn as_ref(&self) -> &str {
        self
    }
}


#[cfg(test)]
mod tests {
    use std::fmt;
    use mime::Mime;
    use mime::TopLevel::Text;
    use mime::SubLevel::Plain;
    use super::{Headers, Header, HeaderFormat, ContentLength, ContentType,
                Accept, Host, qitem};
    use httparse;

    #[cfg(feature = "nightly")]
    use test::Bencher;

    // Slice.position_elem was unstable
    fn index_of(slice: &[u8], byte: u8) -> Option<usize> {
        for (index, &b) in slice.iter().enumerate() {
            if b == byte {
                return Some(index);
            }
        }
        None
    }

    macro_rules! raw {
        ($($line:expr),*) => ({
            [$({
                let line = $line;
                let pos = index_of(line, b':').expect("raw splits on ':', not found");
                httparse::Header {
                    name: ::std::str::from_utf8(&line[..pos]).unwrap(),
                    value: &line[pos + 2..]
                }
            }),*]
        })
    }

    #[test]
    fn test_from_raw() {
        let headers = Headers::from_raw(&raw!(b"Content-Length: 10")).unwrap();
        assert_eq!(headers.get(), Some(&ContentLength(10)));
    }

    #[test]
    fn test_content_type() {
        let content_type = Header::parse_header([b"text/plain".to_vec()].as_ref());
        assert_eq!(content_type.ok(), Some(ContentType(Mime(Text, Plain, vec![]))));
    }

    #[test]
    fn test_accept() {
        let text_plain = qitem(Mime(Text, Plain, vec![]));
        let application_vendor = "application/vnd.github.v3.full+json; q=0.5".parse().unwrap();

        let accept = Header::parse_header([b"text/plain".to_vec()].as_ref());
        assert_eq!(accept.ok(), Some(Accept(vec![text_plain.clone()])));

        let bytevec = [b"application/vnd.github.v3.full+json; q=0.5, text/plain".to_vec()];
        let accept = Header::parse_header(bytevec.as_ref());
        assert_eq!(accept.ok(), Some(Accept(vec![application_vendor, text_plain])));
    }

    #[derive(Clone, PartialEq, Debug)]
    struct CrazyLength(Option<bool>, usize);

    impl Header for CrazyLength {
        fn header_name() -> &'static str {
            "content-length"
        }
        fn parse_header(raw: &[Vec<u8>]) -> ::Result<CrazyLength> {
            use std::str::from_utf8;
            use std::str::FromStr;

            if raw.len() != 1 {
                return Err(::Error::Header);
            }
            // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
            match match from_utf8(unsafe { &raw.get_unchecked(0)[..] }) {
                Ok(s) => FromStr::from_str(s).ok(),
                Err(_) => None
            }.map(|u| CrazyLength(Some(false), u)) {
                Some(x) => Ok(x),
                None => Err(::Error::Header),
            }
        }
    }

    impl HeaderFormat for CrazyLength {
        fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let CrazyLength(ref opt, ref value) = *self;
            write!(f, "{:?}, {:?}", opt, value)
        }
    }

    #[test]
    fn test_different_structs_for_same_header() {
        let headers = Headers::from_raw(&raw!(b"Content-Length: 10")).unwrap();
        assert_eq!(headers.get::<ContentLength>(), Some(&ContentLength(10)));
        assert_eq!(headers.get::<CrazyLength>(), Some(&CrazyLength(Some(false), 10)));
    }

    #[test]
    fn test_trailing_whitespace() {
        let headers = Headers::from_raw(&raw!(b"Content-Length: 10   ")).unwrap();
        assert_eq!(headers.get::<ContentLength>(), Some(&ContentLength(10)));
    }

    #[test]
    fn test_multiple_reads() {
        let headers = Headers::from_raw(&raw!(b"Content-Length: 10")).unwrap();
        let ContentLength(one) = *headers.get::<ContentLength>().unwrap();
        let ContentLength(two) = *headers.get::<ContentLength>().unwrap();
        assert_eq!(one, two);
    }

    #[test]
    fn test_different_reads() {
        let headers = Headers::from_raw(
            &raw!(b"Content-Length: 10", b"Content-Type: text/plain")).unwrap();
        let ContentLength(_) = *headers.get::<ContentLength>().unwrap();
        let ContentType(_) = *headers.get::<ContentType>().unwrap();
    }

    #[test]
    fn test_get_mutable() {
        let mut headers = Headers::from_raw(&raw!(b"Content-Length: 10")).unwrap();
        *headers.get_mut::<ContentLength>().unwrap() = ContentLength(20);
        assert_eq!(headers.get_raw("content-length").unwrap(), &[b"20".to_vec()][..]);
        assert_eq!(*headers.get::<ContentLength>().unwrap(), ContentLength(20));
    }

    #[test]
    fn test_headers_fmt() {
        let mut headers = Headers::new();
        headers.set(ContentLength(15));
        headers.set(Host { hostname: "foo.bar".to_owned(), port: None });

        let s = headers.to_string();
        assert!(s.contains("Host: foo.bar\r\n"));
        assert!(s.contains("Content-Length: 15\r\n"));
    }

    #[test]
    fn test_headers_fmt_raw() {
        let mut headers = Headers::from_raw(&raw!(b"Content-Length: 10")).unwrap();
        headers.set_raw("x-foo", vec![b"foo".to_vec(), b"bar".to_vec()]);
        let s = headers.to_string();
        assert_eq!(s, "Content-Length: 10\r\nx-foo: foo\r\nx-foo: bar\r\n");
    }

    #[test]
    fn test_set_raw() {
        let mut headers = Headers::new();
        headers.set(ContentLength(10));
        headers.set_raw("content-LENGTH", vec![b"20".to_vec()]);
        assert_eq!(headers.get_raw("Content-length").unwrap(), &[b"20".to_vec()][..]);
        assert_eq!(headers.get(), Some(&ContentLength(20)));
    }

    #[test]
    fn test_append_raw() {
        let mut headers = Headers::new();
        headers.set(ContentLength(10));
        headers.append_raw("content-LENGTH", b"20".to_vec());
        assert_eq!(headers.get_raw("Content-length").unwrap(), &[b"10".to_vec(), b"20".to_vec()][..]);
        headers.append_raw("x-foo", b"bar".to_vec());
        assert_eq!(headers.get_raw("x-foo"), Some(&[b"bar".to_vec()][..]));
    }

    #[test]
    fn test_remove_raw() {
        let mut headers = Headers::new();
        headers.set_raw("content-LENGTH", vec![b"20".to_vec()]);
        headers.remove_raw("content-LENGTH");
        assert_eq!(headers.get_raw("Content-length"), None);
    }

    #[test]
    fn test_len() {
        let mut headers = Headers::new();
        headers.set(ContentLength(10));
        assert_eq!(headers.len(), 1);
        headers.set(ContentType(Mime(Text, Plain, vec![])));
        assert_eq!(headers.len(), 2);
        // Redundant, should not increase count.
        headers.set(ContentLength(20));
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut headers = Headers::new();
        headers.set(ContentLength(10));
        headers.set(ContentType(Mime(Text, Plain, vec![])));
        assert_eq!(headers.len(), 2);
        headers.clear();
        assert_eq!(headers.len(), 0);
    }

    #[test]
    fn test_iter() {
        let mut headers = Headers::new();
        headers.set(ContentLength(11));
        for header in headers.iter() {
            assert!(header.is::<ContentLength>());
            assert_eq!(header.name(), <ContentLength as Header>::header_name());
            assert_eq!(header.value(), Some(&ContentLength(11)));
            assert_eq!(header.value_string(), "11".to_owned());
        }
    }

    #[test]
    fn test_header_view_value_string() {
        let mut headers = Headers::new();
        headers.set_raw("foo", vec![b"one".to_vec(), b"two".to_vec()]);
        for header in headers.iter() {
            assert_eq!(header.name(), "foo");
            assert_eq!(header.value_string(), "one, two");
        }
    }

    #[test]
    fn test_eq() {
        let mut headers1 = Headers::new();
        let mut headers2 = Headers::new();

        assert_eq!(headers1, headers2);

        headers1.set(ContentLength(11));
        headers2.set(Host {hostname: "foo.bar".to_owned(), port: None});
        assert!(headers1 != headers2);

        headers1 = Headers::new();
        headers2 = Headers::new();

        headers1.set(ContentLength(11));
        headers2.set(ContentLength(11));
        assert_eq!(headers1, headers2);

        headers1.set(ContentLength(10));
        assert!(headers1 != headers2);

        headers1 = Headers::new();
        headers2 = Headers::new();

        headers1.set(Host { hostname: "foo.bar".to_owned(), port: None });
        headers1.set(ContentLength(11));
        headers2.set(ContentLength(11));
        assert!(headers1 != headers2);
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_new(b: &mut Bencher) {
        b.iter(|| {
            let mut h = Headers::new();
            h.set(ContentLength(11));
            h
        })
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_from_raw(b: &mut Bencher) {
        let raw = raw!(b"Content-Length: 10");
        b.iter(|| Headers::from_raw(&raw).unwrap())
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_get(b: &mut Bencher) {
        let mut headers = Headers::new();
        headers.set(ContentLength(11));
        b.iter(|| assert_eq!(headers.get::<ContentLength>(), Some(&ContentLength(11))))
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_get_miss(b: &mut Bencher) {
        let headers = Headers::new();
        b.iter(|| assert!(headers.get::<ContentLength>().is_none()))
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_set(b: &mut Bencher) {
        let mut headers = Headers::new();
        b.iter(|| headers.set(ContentLength(12)))
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_has(b: &mut Bencher) {
        let mut headers = Headers::new();
        headers.set(ContentLength(11));
        b.iter(|| assert!(headers.has::<ContentLength>()))
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_view_is(b: &mut Bencher) {
        let mut headers = Headers::new();
        headers.set(ContentLength(11));
        let mut iter = headers.iter();
        let view = iter.next().unwrap();
        b.iter(|| assert!(view.is::<ContentLength>()))
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_fmt(b: &mut Bencher) {
        let mut headers = Headers::new();
        headers.set(ContentLength(11));
        b.iter(|| headers.to_string())
    }
}
