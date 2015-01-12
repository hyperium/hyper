use header::{Header, HeaderFormat};
use std::fmt;
use header::shared::util::from_one_raw_str;

/// The `User-Agent` header field.
///
/// They can contain any value, so it just wraps a `String`.
#[derive(Clone, PartialEq, Show)]
pub struct UserAgent(pub String);

deref!(UserAgent => String);

impl Header for UserAgent {
    fn header_name(_: Option<UserAgent>) -> &'static str {
        "User-Agent"
    }

    fn parse_header(raw: &[Vec<u8>]) -> Option<UserAgent> {
        from_one_raw_str(raw).map(|s| UserAgent(s))
    }
}

impl HeaderFormat for UserAgent {
    fn fmt_header(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(&*self.0)
    }
}

bench_header!(bench, UserAgent, { vec![b"cargo bench".to_vec()] });

#[test] fn test_format() {
    use std::borrow::ToOwned;
    use header::Headers;
    let mut head = Headers::new();
    head.set(UserAgent("Bunnies".to_owned()));
    assert!(head.to_string() == "User-Agent: Bunnies\r\n".to_owned());
}
