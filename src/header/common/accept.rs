use header::Header;
use std::fmt::{mod, Show};
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
/// use hyper::mime::{Mime, Text, Html, Xml};
/// # let mut headers = Headers::new();
/// headers.set(Accept(vec![ Mime(Text, Html, vec![]), Mime(Text, Xml, vec![]) ]));
/// ```
#[deriving(Clone, PartialEq, Show)]
pub struct Accept(pub Vec<Mime>);

impl Header for Accept {
    fn header_name(_: Option<Accept>) -> &'static str {
        "accept"
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

