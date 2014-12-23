use header::{Header, HeaderFormat};
use std::fmt::{mod, Show};
use std::str::from_utf8;
use mime::Mime;

/// The `Accept` header.
///
/// The `Accept` header is used to tell a server which content-types the client
/// is capable of using. It can be a comma-separated list of `Mime`s, and the
/// priority can be indicated with a `q` parameter.
///
/// Example:
///
/// ```
/// # use hyper::header::Headers;
/// # use hyper::header::common::Accept;
/// use hyper::mime::Mime;
/// use hyper::mime::TopLevel::Text;
/// use hyper::mime::SubLevel::{Html, Xml};
/// # let mut headers = Headers::new();
/// headers.set(Accept(vec![ Mime(Text, Html, vec![]), Mime(Text, Xml, vec![]) ]));
/// ```
#[deriving(Clone, PartialEq, Show)]
pub struct Accept(pub Vec<Mime>);

deref!(Accept -> Vec<Mime>);

impl Header for Accept {
    fn header_name(_: Option<Accept>) -> &'static str {
        "Accept"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<Accept> {
        let mut mimes: Vec<Mime> = vec![];
        for mimes_raw in raw.iter() {
            match from_utf8(mimes_raw.as_slice()) {
                Ok(mimes_str) => {
                    for mime_str in mimes_str.split(',') {
                        match from_str(mime_str.trim()) {
                            Some(mime) => mimes.push(mime),
                            None => return None
                        }
                    }
                },
                Err(_) => return None
            };
        }

        if !mimes.is_empty() {
            Some(Accept(mimes))
        } else {
            // Currently is just a None, but later it can be Accept for */*
            None
        }
    }
}

impl HeaderFormat for Accept {
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

bench_header!(bench, Accept, { vec![b"text/plain; q=0.5, text/html".to_vec()] });

