use std::fmt::{self, Display};
use header::{self, Header, Raw};

/// `Last-Event-ID` header, defined in
/// [RFC3864](https://html.spec.whatwg.org/multipage/references.html#refsRFC3864)
///
/// The `Last-Event-ID` header contains information about
/// the last event in an http interaction so that it's easier to
/// track of event state. This is helpful when working
/// with [Server-Sent-Events](http://www.html5rocks.com/en/tutorials/eventsource/basics/). If the connection were to be dropped, for example, it'd
/// be useful to let the server know what the last event you
/// received was.
///
/// The spec is a String with the id of the last event, it can be
/// an empty string which acts a sort of "reset".
///
/// # Example
/// ```
/// use hyper::header::{Headers, LastEventId};
///
/// let mut headers = Headers::new();
/// headers.set(LastEventId("1".to_owned()));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct LastEventId(pub String);

impl Header for LastEventId {
    #[inline]
    fn header_name() -> &'static str {
        static NAME: &'static str = "Last-Event-ID";
        NAME
    }

    #[inline]
    fn parse_header(raw: &Raw) -> ::Result<Self> {
        match raw.one() {
            Some(line) if line.is_empty() => Ok(LastEventId("".to_owned())),
            Some(line) => header::parsing::from_raw_str(line).map(LastEventId),
            None => Err(::Error::Header),
        }
    }

    #[inline]
    fn fmt_header(&self, f: &mut header::Formatter) -> fmt::Result {
        f.fmt_line(self)
    }
}

impl Display for LastEventId {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

__hyper__deref!(LastEventId => String);

__hyper__tm!(LastEventId, tests {
    // Initial state
    test_header!(test1, vec![b""]);
    // Own testcase
    test_header!(test2, vec![b"1"], Some(LastEventId("1".to_owned())));
});
