//! Headers container, and common header fields.
//!
//! hyper has the opinion that Headers should be strongly-typed, because that's
//! why we're using Rust in the first place. To set or get any header, an object
//! must implement the `Header` trait from this module. Several common headers
//! are already provided, such as `Host`, `ContentType`, `UserAgent`, and others.
use std::ascii::{AsciiExt, ASCII_LOWER_MAP};
use std::fmt::{mod, Show};
use std::hash;
use std::intrinsics::TypeId;
use std::mem::{transmute, transmute_copy};
use std::raw::TraitObject;
use std::str::{SendStr, Slice, Owned};
use std::collections::hashmap::{HashMap, Entries, Occupied, Vacant};
use std::sync::RWLock;

use uany::{UncheckedAnyDowncast, UncheckedAnyMutDowncast};
use typeable::Typeable;

use http::{read_header, LineEnding};
use {HttpResult};

/// Common Headers
pub mod common;

/// A trait for any object that will represent a header field and value.
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
    /// Format a header to be output into a TcpStream.
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result;
}

#[doc(hidden)]
trait Is {
    fn is<T: 'static>(self) -> bool;
}

impl<'a> Is for &'a Header {
    fn is<T: 'static>(self) -> bool {
        self.get_type() == TypeId::of::<T>()
    }
}

impl<'a> UncheckedAnyDowncast<'a> for &'a Header {
    #[inline]
    unsafe fn downcast_ref_unchecked<T: 'static>(self) -> &'a T {
        let to: TraitObject = transmute_copy(&self);
        transmute(to.data)
    }
}

impl<'a> UncheckedAnyMutDowncast<'a> for &'a mut Header {
    #[inline]
    unsafe fn downcast_mut_unchecked<T: 'static>(self) -> &'a mut T {
        let to: TraitObject = transmute_copy(&self);
        transmute(to.data)
    }
}

fn header_name<T: Header>() -> &'static str {
    let name = Header::header_name(None::<T>);
    name
}

/// A map of header fields on requests and responses.
pub struct Headers {
    data: HashMap<CaseInsensitive, RWLock<Item>>
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
                    let name = CaseInsensitive(Owned(name));
                    let item = match headers.data.entry(name) {
                        Vacant(entry) => entry.set(RWLock::new(Raw(vec![]))),
                        Occupied(entry) => entry.into_mut()
                    };

                    match &mut *item.write() {
                        &Raw(ref mut raw) => raw.push(value),
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
    pub fn set<H: Header>(&mut self, value: H) {
        self.data.insert(CaseInsensitive(Slice(header_name::<H>())), RWLock::new(Typed(box value as Box<Header + Send + Sync>)));
    }

    /// Access the raw value of a header, if it exists and has not
    /// been already parsed.
    ///
    /// If the header field has already been parsed into a typed header,
    /// then you *must* access it through that representation.
    ///
    /// This operation is unsafe because the raw representation can be
    /// invalidated by lasting too long or by the header being parsed
    /// while you still have a reference to the data.
    ///
    /// Example:
    /// ```
    /// # use hyper::header::Headers;
    /// # let mut headers = Headers::new();
    /// let raw_content_type = unsafe { headers.get_raw("content-type") };
    /// ```
    pub unsafe fn get_raw(&self, name: &'static str) -> Option<*const [Vec<u8>]> {
        self.data.find(&CaseInsensitive(Slice(name))).and_then(|item| {
            match *item.read() {
                Raw(ref raw) => Some(raw.as_slice() as *const [Vec<u8>]),
                _ => None
            }
        })
    }

    /// Get a reference to the header field's value, if it exists.
    pub fn get<H: Header>(&self) -> Option<&H> {
        self.get_or_parse::<H>().map(|item| {
            let read = item.read();
            debug!("downcasting {}", *read);
            let ret = match *read {
                Typed(ref val) => unsafe { val.downcast_ref_unchecked() },
                _ => unreachable!()
            };
            unsafe { transmute::<&H, &H>(ret) }
        })
    }

    /// Get a mutable reference to the header field's value, if it exists.
    pub fn get_mut<H: Header>(&mut self) -> Option<&mut H> {
        self.get_or_parse::<H>().map(|item| {
            let mut write = item.write();
            debug!("downcasting {}", *write);
            let ret = match *&mut *write {
                Typed(ref mut val) => unsafe { val.downcast_mut_unchecked() },
                _ => unreachable!()
            };
            unsafe { transmute::<&mut H, &mut H>(ret) }
        })
    }

    fn get_or_parse<H: Header>(&self) -> Option<&RWLock<Item>> {
        self.data.find(&CaseInsensitive(Slice(header_name::<H>()))).and_then(|item| {
            let done = match *item.read() {
                // Huge borrowck hack here, should be refactored to just return here.
                Typed(ref typed) if typed.is::<H>() => true,

                // Typed, wrong type.
                Typed(_) => return None,

                // Raw, work to do.
                Raw(_) => false,
            };

            // borrowck hack continued
            if done { return Some(item); }

            // Take out a write lock to do the parsing and mutation.
            let mut write = item.write();

            let header = match *write {
                // Since this lock can queue, it's possible another thread just
                // did the work for us.
                //
                // Check they inserted the correct type and move on.
                Typed(ref typed) if typed.is::<H>() => return Some(item),

                // Wrong type, another thread got here before us and parsed
                // as a different representation.
                Typed(_) => return None,

                // We are first in the queue or the only ones, so do the actual
                // work of parsing and mutation.
                Raw(ref raw) => match Header::parse_header(raw.as_slice()) {
                    Some::<H>(h) => h,
                    None => return None
                }
            };

            // Mutate in the raw case.
            *write = Typed(box header as Box<Header + Send + Sync>);
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
    pub fn has<H: Header>(&self) -> bool {
        self.data.contains_key(&CaseInsensitive(Slice(header_name::<H>())))
    }

    /// Removes a header from the map, if one existed.
    /// Returns true if a header has been removed.
    pub fn remove<H: Header>(&mut self) -> bool {
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
    inner: Entries<'a, CaseInsensitive, RWLock<Item>>
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
    Typed(Box<Header + Send + Sync>)
}

impl fmt::Show for Item {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Typed(ref h) => h.fmt_header(fmt),
            Raw(ref raw) => {
                for part in raw.iter() {
                    try!(fmt.write(part.as_slice()));
                }
                Ok(())
            },
        }
    }
}

struct CaseInsensitive(SendStr);

impl Str for CaseInsensitive {
    fn as_slice(&self) -> &str {
        let CaseInsensitive(ref s) = *self;
        s.as_slice()
    }

}

impl fmt::Show for CaseInsensitive {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.as_slice().fmt(fmt)
    }
}

impl PartialEq for CaseInsensitive {
    fn eq(&self, other: &CaseInsensitive) -> bool {
        self.as_slice().eq_ignore_ascii_case(other.as_slice())
    }
}

impl Eq for CaseInsensitive {}

impl<H: hash::Writer> hash::Hash<H> for CaseInsensitive {
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
    use super::{Headers, Header};
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

    #[deriving(Clone)]
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
        fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            use std::fmt::Show;
            let CrazyLength(_, ref value) = *self;
            value.fmt(fmt)
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
        headers.set(Host("foo.bar".into_string()));

        let s = headers.to_string();
        // hashmap's iterators have arbitrary order, so we must sort first
        let mut pieces = s[].split_str("\r\n").collect::<Vec<&str>>();
        pieces.sort();
        let s = pieces.into_iter().rev().collect::<Vec<&str>>().connect("\r\n");
        assert_eq!(s[], "Host: foo.bar\r\nContent-Length: 15\r\n");

    }
}

