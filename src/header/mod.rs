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
//! This works well for simple "string" headers.  If you need more control,
//! you can implement the trait directly.
//!
//! ## Implementing the `Header` trait
//!
//! Consider a Do Not Track header. It can be true or false, but it represents
//! that via the numerals `1` and `0`.
//!
//! ```
//! use std::fmt;
//! use hyper::header::{Header, Raw};
//!
//! #[derive(Debug, Clone, Copy)]
//! struct Dnt(bool);
//!
//! impl Header for Dnt {
//!     fn header_name() -> &'static str {
//!         "DNT"
//!     }
//!
//!     fn parse_header(raw: &Raw) -> hyper::Result<Dnt> {
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
//!
//!     fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
//!         if self.0 {
//!             f.write_str("1")
//!         } else {
//!             f.write_str("0")
//!         }
//!     }
//! }
//! ```
use std::any::{Any, TypeId};
use std::borrow::{Cow, ToOwned};
use std::iter::{FromIterator, IntoIterator};
use std::{mem, fmt};

use httparse;
use unicase::UniCase;

use self::internals::{Item, VecMap, Entry};

pub use self::shared::*;
pub use self::common::*;
pub use self::raw::Raw;

mod common;
mod internals;
mod raw;
mod shared;
pub mod parsing;


/// A trait for any object that will represent a header field and value.
///
/// This trait represents the construction and identification of headers,
/// and contains trait-object unsafe methods.
pub trait Header: HeaderClone + Any + GetType + Send + Sync {
    /// Returns the name of the header field this belongs to.
    ///
    /// This will become an associated constant once available.
    fn header_name() -> &'static str where Self: Sized;
    /// Parse a header from a raw stream of bytes.
    ///
    /// It's possible that a request can include a header field more than once,
    /// and in that case, the slice will have a length greater than 1. However,
    /// it's not necessarily the case that a Header is *allowed* to have more
    /// than one field value. If that's the case, you **should** return `None`
    /// if `raw.len() > 1`.
    fn parse_header(raw: &Raw) -> ::Result<Self> where Self: Sized;
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
pub trait GetType: Any {
    #[inline(always)]
    fn get_type(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

impl<T: Any> GetType for T {}

#[test]
fn test_get_type() {
    use ::header::{ContentLength, UserAgent};

    let len = ContentLength(5);
    let agent = UserAgent("hyper".to_owned());

    assert_eq!(TypeId::of::<ContentLength>(), len.get_type());
    assert_eq!(TypeId::of::<UserAgent>(), agent.get_type());

    let len: Box<Header + Send + Sync> = Box::new(len);
    let agent: Box<Header + Send + Sync> = Box::new(agent);

    assert_eq!(TypeId::of::<ContentLength>(), (*len).get_type());
    assert_eq!(TypeId::of::<UserAgent>(), (*agent).get_type());
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

impl<'a, H: Header + ?Sized + 'a> fmt::Display for FmtHeader<'a, H> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt_header(f)
    }
}

struct ValueString<'a>(&'a Item);

impl<'a> fmt::Debug for ValueString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("\""));
        try!(self.0.write_h1(&mut MultilineFormatter(Multi::Join(true, f))));
        f.write_str("\"")
    }
}

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
    fn clone_box(&self) -> Box<Header + Send + Sync>;
}

impl<T: Header + Clone> HeaderClone for T {
    #[inline]
    fn clone_box(&self) -> Box<Header + Send + Sync> {
        Box::new(self.clone())
    }
}

impl Header + Send + Sync {
    // A trait object looks like this:
    //
    // TraitObject { data: *mut (), vtable: *mut () }
    //
    // So, we transmute &Trait into a (*mut (), *mut ()). This depends on the
    // order the compiler has chosen to represent a TraitObject.
    //
    // It has been assured that this order will be stable.
    #[inline]
    unsafe fn downcast_ref_unchecked<T: 'static>(&self) -> &T {
        &*(mem::transmute::<*const _, (*const (), *const ())>(self).0 as *const T)
    }

    #[inline]
    unsafe fn downcast_mut_unchecked<T: 'static>(&mut self) -> &mut T {
        &mut *(mem::transmute::<*mut _, (*mut (), *mut ())>(self).0 as *mut T)
    }

    #[inline]
    unsafe fn downcast_unchecked<T: 'static>(self: Box<Self>) -> T {
        *Box::from_raw(mem::transmute::<*mut _, (*mut (), *mut ())>(Box::into_raw(self)).0 as *mut T)
    }
}

impl Clone for Box<Header + Send + Sync> {
    #[inline]
    fn clone(&self) -> Box<Header + Send + Sync> {
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
    data: VecMap<HeaderName, Item>,
}

impl Default for Headers {
    fn default() -> Headers {
        Headers::new()
    }
}

macro_rules! literals {
    ($($len:expr => $($header:path),+;)+) => (
        fn maybe_literal(s: &str) -> Cow<'static, str> {
            match s.len() {
                $($len => {
                    $(
                    if UniCase(<$header>::header_name()) == s {
                        return Cow::Borrowed(<$header>::header_name());
                    }
                    )+
                })+

                _ => ()
            }

            Cow::Owned(s.to_owned())
        }

        #[test]
        fn test_literal_lens() {
            $(
            $({
                let s = <$header>::header_name();
                assert!(s.len() == $len, "{:?} has len of {}, listed as {}", s, s.len(), $len);
            })+
            )+
        }
    );
}

literals! {
    4  => Host, Date, ETag;
    5  => Allow, Range;
    6  => Accept, Cookie, Server, Expect;
    7  => Upgrade, Referer, Expires;
    8  => Location, IfMatch, IfRange;
    10 => UserAgent, Connection, SetCookie;
    12 => ContentType;
    13 => Authorization<String>, CacheControl, LastModified, IfNoneMatch, AcceptRanges, ContentRange;
    14 => ContentLength, AcceptCharset;
    15 => AcceptEncoding, AcceptLanguage;
    17 => TransferEncoding;
    25 => StrictTransportSecurity;
    27 => AccessControlAllowOrigin;
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
            let name = HeaderName(UniCase(maybe_literal(header.name)));
            let trim = header.value.iter().rev().take_while(|&&x| x == b' ').count();
            let value = &header.value[.. header.value.len() - trim];

            match headers.data.entry(name) {
                Entry::Vacant(entry) => {
                    entry.insert(Item::new_raw(self::raw::parsed(value)));
                }
                Entry::Occupied(entry) => {
                    entry.into_mut().mut_raw().push(value);
                }
            };
        }
        Ok(headers)
    }

    /// Set a header field to the corresponding value.
    ///
    /// The field is determined by the type of the value being set.
    pub fn set<H: Header>(&mut self, value: H) {
        trace!("Headers.set( {:?}, {:?} )", header_name::<H>(), HeaderFormatter(&value));
        self.data.insert(HeaderName(UniCase(Cow::Borrowed(header_name::<H>()))),
                         Item::new_typed(Box::new(value)));
    }

    /// Get a reference to the header field's value, if it exists.
    pub fn get<H: Header>(&self) -> Option<&H> {
        self.data.get(&HeaderName(UniCase(Cow::Borrowed(header_name::<H>()))))
        .and_then(Item::typed::<H>)
    }

    /// Get a mutable reference to the header field's value, if it exists.
    pub fn get_mut<H: Header>(&mut self) -> Option<&mut H> {
        self.data.get_mut(&HeaderName(UniCase(Cow::Borrowed(header_name::<H>()))))
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
    pub fn has<H: Header>(&self) -> bool {
        self.data.contains_key(&HeaderName(UniCase(Cow::Borrowed(header_name::<H>()))))
    }

    /// Removes a header from the map, if one existed.
    /// Returns the header, if one has been removed and could be parsed.
    ///
    /// Note that this function may return `None` even though a header was removed. If you want to
    /// know whether a header exists, rather rely on `has`.
    pub fn remove<H: Header>(&mut self) -> Option<H> {
        trace!("Headers.remove( {:?} )", header_name::<H>());
        self.data.remove(&HeaderName(UniCase(Cow::Borrowed(header_name::<H>()))))
            .and_then(Item::into_typed::<H>)
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

    /// Access the raw value of a header.
    ///
    /// Prefer to use the typed getters instead.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # let mut headers = Headers::new();
    /// # headers.set_raw("content-type", "text/plain");
    /// let raw = headers.get_raw("content-type").unwrap();
    /// assert_eq!(raw, "text/plain");
    /// ```
    pub fn get_raw(&self, name: &str) -> Option<&Raw> {
        self.data
            .get(&HeaderName(UniCase(Cow::Borrowed(unsafe { mem::transmute::<&str, &str>(name) }))))
            .map(Item::raw)
    }

    /// Set the raw value of a header, bypassing any typed headers.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # let mut headers = Headers::new();
    /// headers.set_raw("content-length", b"1".as_ref());
    /// headers.set_raw("content-length", "2");
    /// headers.set_raw("content-length", "3".to_string());
    /// headers.set_raw("content-length", vec![vec![b'4']]);
    /// ```
    pub fn set_raw<K: Into<Cow<'static, str>>, V: Into<Raw>>(&mut self, name: K, value: V) {
        let name = name.into();
        let value = value.into();
        trace!("Headers.set_raw( {:?}, {:?} )", name, value);
        self.data.insert(HeaderName(UniCase(name)), Item::new_raw(value));
    }

    /// Remove a header by name.
    pub fn remove_raw(&mut self, name: &str) {
        trace!("Headers.remove_raw( {:?} )", name);
        self.data.remove(
            &HeaderName(UniCase(Cow::Borrowed(unsafe { mem::transmute::<&str, &str>(name) })))
        );
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
        f.debug_map()
            .entries(self.iter().map(|view| (view.0.as_ref(), ValueString(view.1))))
            .finish()
    }
}

/// An `Iterator` over the fields in a `Headers` map.
#[allow(missing_debug_implementations)]
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
        HeaderName(UniCase(Cow::Borrowed(header_name::<H>()))) == *self.0
    }

    /// Get the Header name as a slice.
    #[inline]
    pub fn name(&self) -> &'a str {
        self.0.as_ref()
    }

    /// Cast the value to a certain Header type.
    #[inline]
    pub fn value<H: Header>(&self) -> Option<&'a H> {
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
        self.1.write_h1(&mut MultilineFormatter(Multi::Line(self.0.as_ref(), f)))
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

impl<'a> fmt::Display for &'a (Header + Send + Sync) {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt_header(f)
    }
}

/// A wrapper around any Header with a Display impl that calls `fmt_header`.
///
/// This can be used like so: `format!("{}", HeaderFormatter(&header))` to
/// get the representation of a Header which will be written to an
/// outgoing `TcpStream`.
pub struct HeaderFormatter<'a, H: Header>(pub &'a H);

impl<'a, H: Header> fmt::Display for HeaderFormatter<'a, H> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt_header(f)
    }
}

impl<'a, H: Header> fmt::Debug for HeaderFormatter<'a, H> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt_header(f)
    }
}

#[derive(Clone, Debug)]
struct HeaderName(UniCase<Cow<'static, str>>);

impl fmt::Display for HeaderName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for HeaderName {
    fn as_ref(&self) -> &str {
        ((self.0).0).as_ref()
    }
}

impl PartialEq for HeaderName {
    fn eq(&self, other: &HeaderName) -> bool {
        let s = self.as_ref();
        let k = other.as_ref();
        if s.len() == k.len() && s.as_ptr() == k.as_ptr() {
            true
        } else {
            self.0 == other.0
        }
    }
}


#[cfg(test)]
mod tests {
    use std::fmt;
    use mime::Mime;
    use mime::TopLevel::Text;
    use mime::SubLevel::Plain;
    use super::{Headers, Header, Raw, ContentLength, ContentType,
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
        let content_type = Header::parse_header(&b"text/plain".as_ref().into());
        assert_eq!(content_type.ok(), Some(ContentType(Mime(Text, Plain, vec![]))));
    }

    #[test]
    fn test_accept() {
        let text_plain = qitem(Mime(Text, Plain, vec![]));
        let application_vendor = "application/vnd.github.v3.full+json; q=0.5".parse().unwrap();

        let accept = Header::parse_header(&b"text/plain".as_ref().into());
        assert_eq!(accept.ok(), Some(Accept(vec![text_plain.clone()])));

        let bytevec = b"application/vnd.github.v3.full+json; q=0.5, text/plain".as_ref().into();
        let accept = Header::parse_header(&bytevec);
        assert_eq!(accept.ok(), Some(Accept(vec![application_vendor, text_plain])));
    }

    #[derive(Clone, PartialEq, Debug)]
    struct CrazyLength(Option<bool>, usize);

    impl Header for CrazyLength {
        fn header_name() -> &'static str {
            "content-length"
        }
        fn parse_header(raw: &Raw) -> ::Result<CrazyLength> {
            use std::str::from_utf8;
            use std::str::FromStr;

            if let Some(line) = raw.one() {
                let s = try!(from_utf8(line).map(|s| FromStr::from_str(s).map_err(|_| ::Error::Header)));
                s.map(|u| CrazyLength(Some(false), u))
            } else {
                Err(::Error::Header)
            }
        }

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
    fn test_headers_to_string() {
        let mut headers = Headers::new();
        headers.set(ContentLength(15));
        headers.set(Host::new("foo.bar", None));

        let s = headers.to_string();
        assert!(s.contains("Host: foo.bar\r\n"));
        assert!(s.contains("Content-Length: 15\r\n"));
    }

    #[test]
    fn test_headers_to_string_raw() {
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
    fn test_remove_raw() {
        let mut headers = Headers::new();
        headers.set_raw("content-LENGTH", vec![b"20".to_vec()]);
        headers.remove_raw("content-LENGTH");
        assert_eq!(headers.get_raw("Content-length"), None);
    }

    #[test]
    fn test_remove() {
        let mut headers = Headers::new();
        headers.set(ContentLength(10));
        assert_eq!(headers.remove(), Some(ContentLength(10)));
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
        headers2.set(Host::new("foo.bar", None));
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

        headers1.set(Host::new("foo.bar", None));
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
