//! Headers container, and common header fields.
//!
//! hyper has the opinion that Headers should be strongly-typed, because that's
//! why we're using Rust in the first place. To set or get any header, an object
//! must implement the `Header` trait from this module. Several common headers
//! are already provided, such as `Host`, `ContentType`, `UserAgent`, and others.
use std::ascii::OwnedAsciiExt;
use std::char::is_lowercase;
use std::fmt::{mod, Show};
use std::mem::{transmute, transmute_copy};
use std::raw::TraitObject;
use std::str::{from_utf8, SendStr, Slice, Owned};
use std::string::raw;
use std::collections::hashmap::{HashMap, Entries};

use uany::UncheckedAnyDowncast;

use http::read_header;
use {HttpResult};

/// Common Headers
pub mod common;

/// A trait for any object that will represent a header field and value.
pub trait Header: 'static {
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

impl<'a> UncheckedAnyDowncast<'a> for &'a Header + 'a {
    #[inline]
    unsafe fn downcast_ref_unchecked<T: 'static>(self) -> &'a T {
        let to: TraitObject = transmute_copy(&self);
        transmute(to.data)
    }
}

fn header_name<T: Header>() -> &'static str {
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
                    }.into_ascii_lower();
                    match headers.data.find_or_insert(Owned(name), Raw(vec![])) {
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
    pub fn set<H: Header>(&mut self, value: H) {
        self.data.insert(Slice(header_name::<H>()), Typed(box value));
    }

    /// Get a clone of the header field's value, if it exists.
    ///
    /// Example:
    ///
    /// ```
    /// # use hyper::header::Headers;
    /// # use hyper::header::common::ContentType;
    /// # let mut headers = Headers::new();
    /// let content_type = headers.get::<ContentType>();
    /// ```
    pub fn get<H: Header + Clone>(&mut self) -> Option<H> {
        self.get_ref().map(|v: &H| v.clone())
    }

    /// Access the raw value of a header, if it exists and has not
    /// been already parsed.
    ///
    /// If the header field has already been parsed into a typed header,
    /// then you *must* access it through that representation.
    ///
    /// Example:
    /// ```
    /// # use hyper::header::Headers;
    /// # let mut headers = Headers::new();
    /// let raw_content_type = unsafe { headers.get_raw("content-type") };
    /// ```
    pub unsafe fn get_raw(&self, name: &'static str) -> Option<&[Vec<u8>]> {
        self.data.find(&Slice(name)).and_then(|item| {
            match *item {
                Raw(ref raw) => Some(raw.as_slice()),
                _ => None
            }
        })
    }

    /// Get a reference to the header field's value, if it exists.
    pub fn get_ref<H: Header>(&mut self) -> Option<&H> {
        self.data.find_mut(&Slice(header_name::<H>())).and_then(|item| {
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
            *item = Typed(box header as Box<Header>);
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
    /// # use hyper::header::Headers;
    /// # use hyper::header::common::ContentType;
    /// # let mut headers = Headers::new();
    /// let has_type = headers.has::<ContentType>();
    /// ```
    pub fn has<H: Header>(&self) -> bool {
        self.data.contains_key(&Slice(header_name::<H>()))
    }

    /// Removes a header from the map, if one existed.
    /// Returns true if a header has been removed.
    pub fn remove<H: Header>(&mut self) -> bool {
        self.data.pop_equiv(&Header::header_name(None::<H>)).is_some()
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

impl<'a> Iterator<(&'a str, HeaderView<'a>)> for HeadersItems<'a> {
    fn next(&mut self) -> Option<(&'a str, HeaderView<'a>)> {
        match self.inner.next() {
            Some((k, v)) => Some((k.as_slice(), HeaderView(v))),
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
    Typed(Box<Header>)
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

#[cfg(test)]
mod tests {
    use std::io::MemReader;
    use mime::{Mime, Text, Plain};
    use super::{Headers, Header};
    use super::common::{ContentLength, ContentType};

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
