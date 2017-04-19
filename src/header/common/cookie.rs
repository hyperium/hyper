use header::{Header, Raw};
use std::fmt::{self, Display};
use std::str::from_utf8;
use std::collections::HashMap;

/// `Cookie` header, defined in [RFC6265](http://tools.ietf.org/html/rfc6265#section-5.4)
///
/// If the user agent does attach a Cookie header field to an HTTP
/// request, the user agent must send the cookie-string
/// as the value of the header field.
///
/// When the user agent generates an HTTP request, the user agent MUST NOT
/// attach more than one Cookie header field.
///
/// # Example values
/// * `SID=31d4d96e407aad42`
/// * `SID=31d4d96e407aad42; lang=en-US`
///
/// # Example
/// ```
/// use hyper::header::{Headers, Cookie};
///
/// let mut headers = Headers::new();
///
/// headers.set(
///    Cookie(vec![
///        String::from("foo=bar")
///    ])
/// );
/// ```
#[derive(Clone, PartialEq, Debug)]
pub struct Cookie(pub Vec<String>);

__hyper__deref!(Cookie => Vec<String>);

impl Header for Cookie {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Cookie";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<Cookie> {
        let mut cookies = Vec::with_capacity(raw.len());
        for cookies_raw in raw.iter() {
            let cookies_str = try!(from_utf8(&cookies_raw[..]));
            for cookie_str in cookies_str.split(';') {
                cookies.push(cookie_str.trim().to_owned())
            }
        }

        if !cookies.is_empty() {
            Ok(Cookie(cookies))
        } else {
            Err(::Error::Header)
        }
    }

    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cookies = &self.0;
        for (i, cookie) in cookies.iter().enumerate() {
            if i != 0 {
                try!(f.write_str("; "));
            }
            try!(Display::fmt(&cookie, f));
        }
        Ok(())

    }
}

impl Cookie {
    /// Returns a HashMap for the cookies in the Cookie header.
    ///
    /// Cookie order is important, if there are duplicate keys, the first
    /// value is used.
    ///
    /// # Example
    ///
    /// ```
    /// // Let's say we got cookie_header from:
    /// // cookie_header = req.headers().get::<hyper::header::Cookie>()
    ///
    /// use hyper::header::Cookie;
    ///
    /// let cookie = Cookie(vec!["SID=1DS38c3R0".to_string(), "datacenter=east01".to_string()]);
    /// let cookie_header = Some(cookie);
    ///
    /// if let Some(ch) = cookie_header {
    ///     let cookie_map = ch.map();
    ///     if let Some(v) = cookie_map.get("datacenter") {
    ///         println!("Got a DC: {}\n", v);
    ///     }
    /// }
    ///
    /// ```
    ///
    pub fn map(&self) -> HashMap<&str, &str> {
        let mut cookie_map = HashMap::with_capacity(self.len());
        for cookie in self.iter() {
            let mut kv_iterator = cookie.splitn(2, '=');
            // split returns at least one element - unwrap is safe
            let k = kv_iterator.next().unwrap().trim();
            let v = match kv_iterator.next() {
                Some(value) => value.trim(),
                None => "",
            };
            // Cookie order is important, the first one prevails when there
            // are duplicates.
            cookie_map.entry(k).or_insert(v);
        }
        cookie_map.shrink_to_fit();
        cookie_map
    }
}

bench_header!(bench, Cookie, {
    vec![b"foo=bar; baz=quux".to_vec()]
});

#[cfg(test)]
mod tests {
    use super::Cookie;
    use std::collections::HashMap;

    #[test]
    fn test_cookie_map_simple() {
        let cookie = Cookie(vec!["a=11 ".to_string(), "  b =  bb".to_string()]);
        let map = cookie.map();
        let good: HashMap<&str, &str> = [("a", "11"), ("b", "bb")].iter().cloned().collect();
        assert_eq!(map, good);
    }

    #[test]
    fn test_cookie_map_duplicate() {
        let cookie =
            Cookie(vec!["a=11 ".to_string(), "  b =  bb".to_string(), "a=22 ".to_string()]);
        let map = cookie.map();
        let good: HashMap<&str, &str> = [("a", "11"), ("b", "bb")].iter().cloned().collect();
        assert_eq!(map, good);
    }
}
