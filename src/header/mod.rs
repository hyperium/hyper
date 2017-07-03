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
//! use hyper::header::{self, Header, Raw};
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
//!     fn fmt_header(&self, f: &mut header::Formatter) -> fmt::Result {
//!         let value = if self.0 {
//!             "1"
//!         } else {
//!             "0"
//!         };
//!         f.fmt_line(&value)
//!     }
//! }
//! ```
use std::borrow::{Cow, ToOwned};
use std::iter::{FromIterator, IntoIterator};
use std::{mem, fmt};

use unicase::Ascii;

use self::internals::{Item, VecMap, Entry};
use self::sealed::{GetType, HeaderClone};

pub use self::shared::*;
pub use self::common::*;
pub use self::raw::Raw;
use bytes::Bytes;

mod common;
mod internals;
mod raw;
mod shared;
pub mod parsing;


/// A trait for any object that will represent a header field and value.
///
/// This trait represents the construction and identification of headers,
/// and contains trait-object unsafe methods.
pub trait Header: HeaderClone + GetType + Send + Sync {
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
    /// Format a header to outgoing stream.
    ///
    /// Most headers should be formatted on one line, and so a common pattern
    /// would be to implement `std::fmt::Display` for this type as well, and
    /// then just call `f.fmt_line(self)`.
    ///
    /// ## Note
    ///
    /// This has the ability to format a header over multiple lines.
    ///
    /// The main example here is `Set-Cookie`, which requires that every
    /// cookie being set be specified in a separate line. Almost every other
    /// case should only format as 1 single line.
    #[inline]
    fn fmt_header(&self, f: &mut Formatter) -> fmt::Result;
}

mod sealed {
    use std::any::{Any, TypeId};
    use super::Header;

    #[doc(hidden)]
    pub trait GetType: Any {
        #[inline(always)]
        fn get_type(&self) -> TypeId {
            TypeId::of::<Self>()
        }
    }

    impl<T: Any> GetType for T {}

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

    #[test]
    fn test_get_type() {
        use ::header::{ContentLength, UserAgent};

        let len = ContentLength(5);
        let agent = UserAgent::new("hyper");

        assert_eq!(TypeId::of::<ContentLength>(), len.get_type());
        assert_eq!(TypeId::of::<UserAgent>(), agent.get_type());

        let len: Box<Header + Send + Sync> = Box::new(len);
        let agent: Box<Header + Send + Sync> = Box::new(agent);

        assert_eq!(TypeId::of::<ContentLength>(), (*len).get_type());
        assert_eq!(TypeId::of::<UserAgent>(), (*agent).get_type());
    }
}


/// A formatter used to serialize headers to an output stream.
#[allow(missing_debug_implementations)]
pub struct Formatter<'a, 'b: 'a>(Multi<'a, 'b>);

enum Multi<'a, 'b: 'a> {
    Line(&'a str, &'a mut fmt::Formatter<'b>),
    Join(bool, &'a mut fmt::Formatter<'b>),
    Raw(&'a mut Raw),
}

impl<'a, 'b> Formatter<'a, 'b> {

    /// Format one 'line' of a header.
    ///
    /// This writes the header name plus the `Display` value as a single line.
    ///
    /// ## Note
    ///
    /// This has the ability to format a header over multiple lines.
    ///
    /// The main example here is `Set-Cookie`, which requires that every
    /// cookie being set be specified in a separate line. Almost every other
    /// case should only format as 1 single line.
    pub fn fmt_line(&mut self, line: &fmt::Display) -> fmt::Result {
        use std::fmt::Write;
        match self.0 {
            Multi::Line(name, ref mut f) => {
                try!(f.write_str(name));
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
            Multi::Raw(ref mut raw) => {
                let mut s = String::new();
                try!(write!(NewlineReplacer(&mut s), "{}", line));
                raw.push(s);
                Ok(())
            }
        }
    }

    fn danger_fmt_line_without_newline_replacer<T: fmt::Display>(&mut self, line: &T) -> fmt::Result {
        use std::fmt::Write;
        match self.0 {
            Multi::Line(name, ref mut f) => {
                try!(f.write_str(name));
                try!(f.write_str(": "));
                try!(fmt::Display::fmt(line, f));
                f.write_str("\r\n")
            },
            Multi::Join(ref mut first, ref mut f) => {
                if !*first {
                    try!(f.write_str(", "));
                } else {
                    *first = false;
                }
                fmt::Display::fmt(line, f)
            }
            Multi::Raw(ref mut raw) => {
                let mut s = String::new();
                try!(write!(s, "{}", line));
                raw.push(s);
                Ok(())
            }
        }
    }
}

struct ValueString<'a>(&'a Item);

impl<'a> fmt::Debug for ValueString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("\""));
        try!(self.0.write_h1(&mut Formatter(Multi::Join(true, f))));
        f.write_str("\"")
    }
}

impl<'a> fmt::Display for ValueString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.write_h1(&mut Formatter(Multi::Join(true, f)))
    }
}

struct HeaderValueString<'a, H: Header + 'a>(&'a H);

impl<'a, H: Header> fmt::Debug for HeaderValueString<'a, H> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(f.write_str("\""));
        try!(self.0.fmt_header(&mut Formatter(Multi::Join(true, f))));
        f.write_str("\"")
    }
}

struct NewlineReplacer<'a, F: fmt::Write + 'a>(&'a mut F);

impl<'a, F: fmt::Write + 'a> fmt::Write for NewlineReplacer<'a, F> {
    #[inline]
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

    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
        fmt::write(self, args)
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
                    if Ascii::new(<$header>::header_name()) == Ascii::new(s) {
                        return Cow::Borrowed(<$header>::header_name());
                    }
                    )+
                })+

                _ => ()
            }

            trace!("maybe_literal not found, copying {:?}", s);
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
    #[inline]
    pub fn new() -> Headers {
        Headers::with_capacity(0)
    }

    /// Creates a new `Headers` struct with space reserved for `len` headers.
    #[inline]
    pub fn with_capacity(len: usize) -> Headers {
        Headers {
            data: VecMap::with_capacity(len)
        }
    }

    /// Set a header field to the corresponding value.
    ///
    /// The field is determined by the type of the value being set.
    pub fn set<H: Header>(&mut self, value: H) {
        trace!("Headers.set( {:?}, {:?} )", header_name::<H>(), HeaderValueString(&value));
        self.data.insert(HeaderName(Ascii::new(Cow::Borrowed(header_name::<H>()))),
                         Item::new_typed(value));
    }

    /// Get a reference to the header field's value, if it exists.
    pub fn get<H: Header>(&self) -> Option<&H> {
        self.data.get(&HeaderName(Ascii::new(Cow::Borrowed(header_name::<H>()))))
        .and_then(Item::typed::<H>)
    }

    /// Get a mutable reference to the header field's value, if it exists.
    pub fn get_mut<H: Header>(&mut self) -> Option<&mut H> {
        self.data.get_mut(&HeaderName(Ascii::new(Cow::Borrowed(header_name::<H>()))))
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
    /// headers.set(ContentType::json());
    /// assert!(headers.has::<ContentType>());
    /// ```
    pub fn has<H: Header>(&self) -> bool {
        self.data.contains_key(&HeaderName(Ascii::new(Cow::Borrowed(header_name::<H>()))))
    }

    /// Removes a header from the map, if one existed.
    /// Returns the header, if one has been removed and could be parsed.
    ///
    /// Note that this function may return `None` even though a header was removed. If you want to
    /// know whether a header exists, rather rely on `has`.
    pub fn remove<H: Header>(&mut self) -> Option<H> {
        trace!("Headers.remove( {:?} )", header_name::<H>());
        self.data.remove(&HeaderName(Ascii::new(Cow::Borrowed(header_name::<H>()))))
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
            .get(name)
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
        self.data.insert(HeaderName(Ascii::new(name)), Item::new_raw(value));
    }

    /// Append a value to raw value of this header.
    ///
    /// If a header already contains a value, this will add another line to it.
    ///
    /// If a header does not exist for this name, a new one will be created with
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
    pub fn append_raw<K: Into<Cow<'static, str>>, V: Into<Raw>>(&mut self, name: K, value: V) {
        let name = name.into();
        let value = value.into();
        trace!("Headers.append_raw( {:?}, {:?} )", name, value);
        let name = HeaderName(Ascii::new(name));
        if let Some(item) = self.data.get_mut(&name) {
            item.raw_mut().push(value);
            return;
        }
        self.data.insert(name, Item::new_raw(value));
    }

    /// Remove a header by name.
    pub fn remove_raw(&mut self, name: &str) {
        trace!("Headers.remove_raw( {:?} )", name);
        self.data.remove(name);
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
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for header in self.iter() {
            try!(fmt::Display::fmt(&header, f));
        }
        Ok(())
    }
}

impl fmt::Debug for Headers {
    #[inline]
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
        HeaderName(Ascii::new(Cow::Borrowed(header_name::<H>()))) == *self.0
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

    /// Access the raw value of the header.
    #[inline]
    pub fn raw(&self) -> &Raw {
        self.1.raw()
    }
}

impl<'a> fmt::Display for HeaderView<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.1.write_h1(&mut Formatter(Multi::Line(self.0.as_ref(), f)))
    }
}

impl<'a> fmt::Debug for HeaderView<'a> {
    #[inline]
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

impl<'a> Extend<(&'a str, Bytes)> for Headers {
    fn extend<I: IntoIterator<Item=(&'a str, Bytes)>>(&mut self, iter: I) {
        for (name, value) in iter {
            let name = HeaderName(Ascii::new(maybe_literal(name)));
            //let trim = header.value.iter().rev().take_while(|&&x| x == b' ').count();

            match self.data.entry(name) {
                Entry::Vacant(entry) => {
                    entry.insert(Item::new_raw(self::raw::parsed(value)));
                }
                Entry::Occupied(entry) => {
                    self::raw::push(entry.into_mut().raw_mut(), value);
                }
            };
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

#[derive(Clone, Debug)]
struct HeaderName(Ascii<Cow<'static, str>>);

impl fmt::Display for HeaderName {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.0.as_ref(), f)
    }
}

impl AsRef<str> for HeaderName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl PartialEq for HeaderName {
    #[inline]
    fn eq(&self, other: &HeaderName) -> bool {
        let s = self.as_ref();
        let k = other.as_ref();
        if s.as_ptr() == k.as_ptr() && s.len() == k.len() {
            true
        } else {
            self.0 == other.0
        }
    }
}

impl PartialEq<HeaderName> for str {
    fn eq(&self, other: &HeaderName) -> bool {
        let k = other.as_ref();
        if self.as_ptr() == k.as_ptr() && self.len() == k.len() {
            true
        } else {
            other.0 == self
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use super::{Headers, Header, Raw, ContentLength, ContentType, Host, SetCookie};

    #[cfg(feature = "nightly")]
    use test::Bencher;

    macro_rules! make_header {
        ($name:expr, $value:expr) => ({
            let mut headers = Headers::new();
            headers.set_raw(String::from_utf8($name.to_vec()).unwrap(), $value.to_vec());
            headers
        });
        ($text:expr) => ({
            let bytes = $text;
            let colon = bytes.iter().position(|&x| x == b':').unwrap();
            make_header!(&bytes[..colon], &bytes[colon + 2..])
        })
    }
    #[test]
    fn test_from_raw() {
        let headers = make_header!(b"Content-Length", b"10");
        assert_eq!(headers.get(), Some(&ContentLength(10)));
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

        fn fmt_header(&self, f: &mut super::Formatter) -> fmt::Result {
            f.fmt_line(self)
        }
    }

    impl fmt::Display for CrazyLength {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let CrazyLength(ref opt, ref value) = *self;
            write!(f, "{:?}, {:?}", opt, value)
        }
    }

    #[test]
    fn test_different_structs_for_same_header() {
        let headers = make_header!(b"Content-Length: 10");
        assert_eq!(headers.get::<ContentLength>(), Some(&ContentLength(10)));
        assert_eq!(headers.get::<CrazyLength>(), Some(&CrazyLength(Some(false), 10)));
    }

    #[test]
    fn test_trailing_whitespace() {
        let headers = make_header!(b"Content-Length: 10   ");
        assert_eq!(headers.get::<ContentLength>(), Some(&ContentLength(10)));
    }

    #[test]
    fn test_multiple_reads() {
        let headers = make_header!(b"Content-Length: 10");
        let ContentLength(one) = *headers.get::<ContentLength>().unwrap();
        let ContentLength(two) = *headers.get::<ContentLength>().unwrap();
        assert_eq!(one, two);
    }

    #[test]
    fn test_different_reads() {
        let mut headers = Headers::new();
        headers.set_raw("Content-Length", "10");
        headers.set_raw("Content-Type", "text/plain");
        let ContentLength(_) = *headers.get::<ContentLength>().unwrap();
        let ContentType(_) = *headers.get::<ContentType>().unwrap();
    }

    #[test]
    fn test_typed_get_raw() {
        let mut headers = Headers::new();
        headers.set(ContentLength(15));
        assert_eq!(headers.get_raw("content-length").unwrap(), "15");

        headers.set(SetCookie(vec![
            "foo=bar".to_string(),
            "baz=quux; Path=/path".to_string()
        ]));
        assert_eq!(headers.get_raw("set-cookie").unwrap(), &["foo=bar", "baz=quux; Path=/path"][..]);
    }

    #[test]
    fn test_get_mutable() {
        let mut headers = make_header!(b"Content-Length: 10");
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
        let mut headers = make_header!(b"Content-Length: 10");
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
        headers.append_raw("x-foo", "bar");
        assert_eq!(headers.get_raw("x-foo").unwrap(), &[b"bar".to_vec()][..]);
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
        headers.set(ContentType::json());
        assert_eq!(headers.len(), 2);
        // Redundant, should not increase count.
        headers.set(ContentLength(20));
        assert_eq!(headers.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut headers = Headers::new();
        headers.set(ContentLength(10));
        headers.set(ContentType::json());
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
    fn test_header_view_raw() {
        let mut headers = Headers::new();
        headers.set_raw("foo", vec![b"one".to_vec(), b"two".to_vec()]);
        for header in headers.iter() {
            assert_eq!(header.name(), "foo");
            let values: Vec<&[u8]> = header.raw().iter().collect();
            assert_eq!(values, vec![b"one", b"two"]);
        }
    }

    #[test]
    fn test_eq() {
        let mut headers1 = Headers::new();
        let mut headers2 = Headers::new();

        assert_eq!(headers1, headers2);

        headers1.set(ContentLength(11));
        headers2.set(Host::new("foo.bar", None));
        assert_ne!(headers1, headers2);

        headers1 = Headers::new();
        headers2 = Headers::new();

        headers1.set(ContentLength(11));
        headers2.set(ContentLength(11));
        assert_eq!(headers1, headers2);

        headers1.set(ContentLength(10));
        assert_ne!(headers1, headers2);

        headers1 = Headers::new();
        headers2 = Headers::new();

        headers1.set(Host::new("foo.bar", None));
        headers1.set(ContentLength(11));
        headers2.set(ContentLength(11));
        assert_ne!(headers1, headers2);
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
    fn bench_headers_get_miss_previous_10(b: &mut Bencher) {
        let mut headers = Headers::new();
        for i in 0..10 {
            headers.set_raw(format!("non-standard-{}", i), "hi");
        }
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
    fn bench_headers_set_previous_10(b: &mut Bencher) {
        let mut headers = Headers::new();
        for i in 0..10 {
            headers.set_raw(format!("non-standard-{}", i), "hi");
        }
        b.iter(|| headers.set(ContentLength(12)))
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_set_raw(b: &mut Bencher) {
        let mut headers = Headers::new();
        b.iter(|| headers.set_raw("non-standard", "hello"))
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_headers_set_raw_previous_10(b: &mut Bencher) {
        let mut headers = Headers::new();
        for i in 0..10 {
            headers.set_raw(format!("non-standard-{}", i), "hi");
        }
        b.iter(|| headers.set_raw("non-standard", "hello"))
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
        use std::fmt::Write;
        let mut buf = String::with_capacity(64);
        let mut headers = Headers::new();
        headers.set(ContentLength(11));
        headers.set(ContentType::json());
        b.bytes = headers.to_string().len() as u64;
        b.iter(|| {
            let _ = write!(buf, "{}", headers);
            ::test::black_box(&buf);
            unsafe { buf.as_mut_vec().set_len(0); }
        })
    }
}
