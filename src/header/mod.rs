//! Headers container, and common header fields.
//!
//! hyper has the opinion that Headers should be strongly-typed, because that's
//! why we're using Rust in the first place. To set or get any header, an object
//! must implement the `Header` trait from this module. Several common headers
//! are already provided, such as `Host`, `ContentType`, `UserAgent`, and others.
use std::ascii::{AsciiExt, ASCII_LOWER_MAP};
use std::fmt::{mod, Show};
use std::intrinsics::TypeId;
use std::raw::TraitObject;
use std::str::{SendStr, Slice, Owned};
use std::collections::hashmap::{HashMap, Entries, Occupied, Vacant};
use std::sync::RWLock;
use std::{hash, mem};

use uany::{UncheckedAnyDowncast, UncheckedAnyMutDowncast};
use typeable::Typeable;

use http::{mod, LineEnding};
use {HttpResult};

/// Common Headers
pub mod common;

/// A trait for any object that will represent a header field and value.
///
/// This trait represents the construction and identification of headers,
/// and contains trait-object unsafe methods.
pub trait Header: Typeable + Send + Sync {
    /// Returns the name of the header field this belongs to.
    ///
    /// The market `Option` is to hint to the type system which implementation
    /// to call. This can be done away with once UFCS arrives.
    fn header_name(marker: Option<Self>) -> &'static str;
    /// Parse a header from a raw stream of bytes.
    ///
    /// It's possible that a request can include a header field more than once,
    /// and in that case, the slice will have a length greater than 1. However,
    /// it's not necessarily the case that a Header is *allowed* to have more
    /// than one field value. If that's the case, you **should** return `None`
    /// if `raw.len() > 1`.
    fn parse_header(raw: &[Vec<u8>]) -> Option<Self>;
}

/// A trait for any object that will represent a header field and value.
///
/// This trait represents the formatting of a Header for output to a TcpStream.
pub trait HeaderFormat: Typeable + Send + Sync {
    /// Format a header to be output into a TcpStream.
    ///
    /// This method is not allowed to introduce an Err not produced
    /// by the passed-in Formatter.
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result;
}

#[doc(hidden)]
trait Is {
    fn is<T: 'static>(self) -> bool;
}

impl<'a> Is for &'a HeaderFormat {
    fn is<T: 'static>(self) -> bool {
        self.get_type() == TypeId::of::<T>()
    }
}

impl<'a> UncheckedAnyDowncast<'a> for &'a HeaderFormat {
    #[inline]
    unsafe fn downcast_ref_unchecked<T: 'static>(self) -> &'a T {
        let to: TraitObject = mem::transmute_copy(&self);
        mem::transmute(to.data)
    }
}

impl<'a> UncheckedAnyMutDowncast<'a> for &'a mut HeaderFormat {
    #[inline]
    unsafe fn downcast_mut_unchecked<T: 'static>(self) -> &'a mut T {
        let to: TraitObject = mem::transmute_copy(&self);
        mem::transmute(to.data)
    }
}

fn header_name<T: Header + HeaderFormat>() -> &'static str {
    let name = Header::header_name(None::<T>);
    name
}

/// A map of header fields on requests and responses.
pub struct Headers {
    data: HashMap<CaseInsensitive<SendStr>, RWLock<Item>>
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
            match try!(http::read_header(rdr)) {
                Some((name, value)) => {
                    let name = CaseInsensitive(Owned(name));
                    let item = match headers.data.entry(name) {
                        Vacant(entry) => entry.set(RWLock::new(Item::raw(vec![]))),
                        Occupied(entry) => entry.into_mut()
                    };

                    match &mut item.write().raw {
                        &Some(ref mut raw) => raw.push(value),
                        // Unreachable
                        _ => {}
                    };
                },
                None => break,
            }
        }
        Ok(headers)
    }

    /// Set a header field to the corresponding value.
    ///
    /// The field is determined by the type of the value being set.
    pub fn set<H: Header + HeaderFormat>(&mut self, value: H) {
        self.data.insert(CaseInsensitive(Slice(header_name::<H>())),
                         RWLock::new(Item::typed(box value as Box<HeaderFormat + Send + Sync>)));
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
        self.data.find_equiv(&CaseInsensitive(name)).and_then(|item| {
            let lock = item.read();
            if let Some(ref raw) = lock.raw {
                return unsafe { mem::transmute(Some(raw[])) };
            }

            let mut lock = item.write();
            let raw = vec![lock.typed.as_ref().unwrap().to_string().into_bytes()];
            lock.raw = Some(raw);
            unsafe { mem::transmute(Some(lock.raw.as_ref().unwrap()[])) }
        })
    }

    /// Set the raw value of a header, bypassing any typed headers.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # let mut headers = Headers::new();
    /// headers.set_raw("content-length", vec!["5".as_bytes().to_vec()]);
    /// ```
    pub fn set_raw<K: IntoMaybeOwned<'static>>(&mut self, name: K, value: Vec<Vec<u8>>) {
        self.data.insert(CaseInsensitive(name.into_maybe_owned()), RWLock::new(Item::raw(value)));
    }

    /// Get a reference to the header field's value, if it exists.
    pub fn get<H: Header + HeaderFormat>(&self) -> Option<&H> {
        self.get_or_parse::<H>().map(|item| {
            let read = item.read();
            debug!("downcasting {}", *read);
            let ret = match read.typed {
                Some(ref val) => unsafe { val.downcast_ref_unchecked() },
                _ => unreachable!()
            };
            unsafe { mem::transmute::<&H, &H>(ret) }
        })
    }

    /// Get a mutable reference to the header field's value, if it exists.
    pub fn get_mut<H: Header + HeaderFormat>(&mut self) -> Option<&mut H> {
        self.get_or_parse::<H>().map(|item| {
            let mut write = item.write();
            debug!("downcasting {}", *write);
            let ret = match *&mut write.typed {
                Some(ref mut val) => unsafe { val.downcast_mut_unchecked() },
                _ => unreachable!()
            };
            unsafe { mem::transmute::<&mut H, &mut H>(ret) }
        })
    }

    fn get_or_parse<H: Header + HeaderFormat>(&self) -> Option<&RWLock<Item>> {
        self.data.find(&CaseInsensitive(Slice(header_name::<H>()))).and_then(|item| {
            match item.read().typed {
                Some(ref typed) if typed.is::<H>() => return Some(item),
                Some(ref typed) => {
                    warn!("attempted to access {} as wrong type", typed);
                    return None;
                }
                _ => ()
            }

            // Take out a write lock to do the parsing and mutation.
            let mut write = item.write();

            // Since this lock can queue, it's possible another thread just
            // did the work for us.
            match write.typed {
                // Check they inserted the correct type and move on.
                Some(ref typed) if typed.is::<H>() => return Some(item),

                // Wrong type, another thread got here before us and parsed
                // as a different representation.
                Some(ref typed) => {
                    debug!("other thread was here first?")
                    warn!("attempted to access {} as wrong type", typed);
                    return None;
                },

                // We are first in the queue or the only ones, so do the actual
                // work of parsing and mutation.
                _ => ()
            }

            let header = match write.raw {
                Some(ref raw) => match Header::parse_header(raw[]) {
                    Some::<H>(h) => h,
                    None => return None
                },
                None => unreachable!()
            };

            // Mutate!
            write.typed = Some(box header as Box<HeaderFormat + Send + Sync>);
            Some(item)
        })
    }

    /// Returns a boolean of whether a certain header is in the map.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # use hyper::header::common::ContentType;
    /// # let mut headers = Headers::new();
    /// let has_type = headers.has::<ContentType>();
    /// ```
    pub fn has<H: Header + HeaderFormat>(&self) -> bool {
        self.data.contains_key(&CaseInsensitive(Slice(header_name::<H>())))
    }

    /// Removes a header from the map, if one existed.
    /// Returns true if a header has been removed.
    pub fn remove<H: Header + HeaderFormat>(&mut self) -> bool {
        self.data.remove(&CaseInsensitive(Slice(Header::header_name(None::<H>))))
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
        for (k, v) in self.iter() {
            try!(write!(fmt, "{}: {}{}", k, v, LineEnding));
        }
        Ok(())
    }
}

/// An `Iterator` over the fields in a `Headers` map.
pub struct HeadersItems<'a> {
    inner: Entries<'a, CaseInsensitive<SendStr>, RWLock<Item>>
}

impl<'a> Iterator<(&'a str, HeaderView<'a>)> for HeadersItems<'a> {
    fn next(&mut self) -> Option<(&'a str, HeaderView<'a>)> {
        match self.inner.next() {
            Some((k, v)) => Some((k.as_slice(), HeaderView(v))),
            None => None
        }
    }
}

/// Returned with the `HeadersItems` iterator.
pub struct HeaderView<'a>(&'a RWLock<Item>);

impl<'a> fmt::Show for HeaderView<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let HeaderView(item) = *self;
        item.read().fmt(fmt)
    }
}

struct Item {
    raw: Option<Vec<Vec<u8>>>,
    typed: Option<Box<HeaderFormat + Send + Sync>>
}

impl Item {
    fn raw(data: Vec<Vec<u8>>) -> Item {
        Item {
            raw: Some(data),
            typed: None,
        }
    }

    fn typed(ty: Box<HeaderFormat + Send + Sync>) -> Item {
        Item {
            raw: None,
            typed: Some(ty),
        }
    }
}

impl fmt::Show for Item {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.typed {
            Some(ref h) => h.fmt_header(fmt),
            None => match self.raw {
                Some(ref raw) => {
                    for part in raw.iter() {
                        try!(fmt.write(part.as_slice()));
                    }
                    Ok(())
                },
                None => unreachable!()
            }
        }
    }
}

impl fmt::Show for Box<HeaderFormat + Send + Sync> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt_header(fmt)
    }
}

struct CaseInsensitive<S: Str>(S);

impl<S: Str> Str for CaseInsensitive<S> {
    fn as_slice(&self) -> &str {
        let CaseInsensitive(ref s) = *self;
        s.as_slice()
    }

}

impl<S: Str> fmt::Show for CaseInsensitive<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.as_slice().fmt(fmt)
    }
}

impl<S: Str> PartialEq for CaseInsensitive<S> {
    fn eq(&self, other: &CaseInsensitive<S>) -> bool {
        self.as_slice().eq_ignore_ascii_case(other.as_slice())
    }
}

impl<S: Str> Eq for CaseInsensitive<S> {}

impl<S: Str, S2: Str> Equiv<CaseInsensitive<S2>> for CaseInsensitive<S> {
    fn equiv(&self, other: &CaseInsensitive<S2>) -> bool {
        let left = CaseInsensitive(self.as_slice());
        let right = CaseInsensitive(other.as_slice());
        left == right
    }
}

impl<S: Str, H: hash::Writer> hash::Hash<H> for CaseInsensitive<S> {
    #[inline]
    fn hash(&self, hasher: &mut H) {
        for byte in self.as_slice().bytes() {
            hasher.write([ASCII_LOWER_MAP[byte as uint]].as_slice());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::MemReader;
    use std::fmt;
    use std::str::Slice;
    use std::hash::sip::hash;
    use mime::{Mime, Text, Plain};
    use super::CaseInsensitive;
    use super::{Headers, Header, HeaderFormat};
    use super::common::{ContentLength, ContentType, Accept, Host};

    fn mem(s: &str) -> MemReader {
        MemReader::new(s.as_bytes().to_vec())
    }

    #[test]
    fn test_case_insensitive() {
        let a = CaseInsensitive(Slice("foobar"));
        let b = CaseInsensitive(Slice("FOOBAR"));

        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
    }

    #[test]
    fn test_from_raw() {
        let headers = Headers::from_raw(&mut mem("Content-Length: 10\r\n\r\n")).unwrap();
        assert_eq!(headers.get(), Some(&ContentLength(10)));
    }

    #[test]
    fn test_content_type() {
        let content_type = Header::parse_header(["text/plain".as_bytes().to_vec()].as_slice());
        assert_eq!(content_type, Some(ContentType(Mime(Text, Plain, vec![]))));
    }

    #[test]
    fn test_accept() {
        let text_plain = Mime(Text, Plain, vec![]);
        let application_vendor = from_str("application/vnd.github.v3.full+json; q=0.5").unwrap();

        let accept = Header::parse_header([b"text/plain".to_vec()].as_slice());
        assert_eq!(accept, Some(Accept(vec![text_plain.clone()])));

        let accept = Header::parse_header([b"application/vnd.github.v3.full+json; q=0.5, text/plain".to_vec()].as_slice());
        assert_eq!(accept, Some(Accept(vec![application_vendor, text_plain])));
    }

    #[deriving(Clone, Show)]
    struct CrazyLength(Option<bool>, uint);

    impl Header for CrazyLength {
        fn header_name(_: Option<CrazyLength>) -> &'static str {
            "content-length"
        }
        fn parse_header(raw: &[Vec<u8>]) -> Option<CrazyLength> {
            use std::str::from_utf8;
            use std::from_str::FromStr;

            if raw.len() != 1 {
                return None;
            }
            // we JUST checked that raw.len() == 1, so raw[0] WILL exist.
            match from_utf8(unsafe { raw.as_slice().unsafe_get(0).as_slice() }) {
                Some(s) => FromStr::from_str(s),
                None => None
            }.map(|u| CrazyLength(Some(false), u))
        }
    }

    impl HeaderFormat for CrazyLength {
        fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            let CrazyLength(ref opt, ref value) = *self;
            write!(fmt, "{}, {}", opt, value)
        }
    }

    #[test]
    fn test_different_structs_for_same_header() {
        let headers = Headers::from_raw(&mut mem("Content-Length: 10\r\n\r\n")).unwrap();
        let ContentLength(_) = *headers.get::<ContentLength>().unwrap();
        assert!(headers.get::<CrazyLength>().is_none());
    }

    #[test]
    fn test_multiple_reads() {
        let headers = Headers::from_raw(&mut mem("Content-Length: 10\r\n\r\n")).unwrap();
        let ContentLength(one) = *headers.get::<ContentLength>().unwrap();
        let ContentLength(two) = *headers.get::<ContentLength>().unwrap();
        assert_eq!(one, two);
    }

    #[test]
    fn test_different_reads() {
        let headers = Headers::from_raw(&mut mem("Content-Length: 10\r\nContent-Type: text/plain\r\n\r\n")).unwrap();
        let ContentLength(_) = *headers.get::<ContentLength>().unwrap();
        let ContentType(_) = *headers.get::<ContentType>().unwrap();
    }

    #[test]
    fn test_get_mutable() {
        let mut headers = Headers::from_raw(&mut mem("Content-Length: 10\r\nContent-Type: text/plain\r\n\r\n")).unwrap();
        *headers.get_mut::<ContentLength>().unwrap() = ContentLength(20);
        assert_eq!(*headers.get::<ContentLength>().unwrap(), ContentLength(20));
    }

    #[test]
    fn test_headers_show() {
        let mut headers = Headers::new();
        headers.set(ContentLength(15));
        headers.set(Host { hostname: "foo.bar".into_string(), port: None });

        let s = headers.to_string();
        // hashmap's iterators have arbitrary order, so we must sort first
        let mut pieces = s[].split_str("\r\n").collect::<Vec<&str>>();
        pieces.sort();
        let s = pieces.into_iter().rev().collect::<Vec<&str>>().connect("\r\n");
        assert_eq!(s[], "Host: foo.bar\r\nContent-Length: 15\r\n");
    }

    #[test]
    fn test_set_raw() {
        let mut headers = Headers::new();
        headers.set(ContentLength(10));
        headers.set_raw("content-LENGTH", vec![b"20".to_vec()]);
        assert_eq!(headers.get_raw("Content-length").unwrap(), [b"20".to_vec()][]);
        assert_eq!(headers.get(), Some(&ContentLength(20)));
    }
}

