use header::{Header, Raw, VecMap};
use std::fmt;
use std::str::from_utf8;
use std::borrow::Borrow;

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
#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct Cookie {
    cookies: Vec<(String, String)>,
    index: VecMap<String, String>,
}


//__hyper__deref!(Cookie => Vec<String>);

impl Header for Cookie {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Cookie";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<Cookie> {
        let mut cookies = Cookie::with_capacity(raw.len());
        for cookies_raw in raw.iter() {
            let cookies_str = try!(from_utf8(&cookies_raw[..]));
            for cookie_str in cookies_str.split(';') {
                let cookie_trimmed = cookie_str.trim();
                if cookie_trimmed.len() == 0 {
                    continue;
                }
                let mut kv_iterator = cookie_trimmed.splitn(2, '=');
                // split returns at least one element - unwrap is safe
                let k = kv_iterator.next().unwrap().trim();
                let v = match kv_iterator.next() { 
                    Some(value) => value.trim(),
                    None => "", 
                };

                cookies.push(k, v);
            }
        }
        cookies.shrink_to_fit();
        if !cookies.is_empty() {
            Ok(cookies)
        } else {
            Err(::Error::Header)
        }
    }

    fn fmt_header(&self, f: &mut ::header::Formatter) -> fmt::Result {
        f.fmt_line(self)
    }
}

impl Cookie {
    /// Create an empty Cookie header.
    pub fn new() -> Cookie {
        Cookie::with_capacity(0)
    }

    /// Create a Cookie header of a certain size.
    pub fn with_capacity(capacity: usize) -> Cookie {
        Cookie {
            cookies: Vec::with_capacity(capacity),
            index: VecMap::with_capacity(capacity),
        }
    }

    /// Shrink the Cookie header internal elements to the currently used size.
    pub fn shrink_to_fit(&mut self) {
        self.cookies.shrink_to_fit();
    }

    /// Append a new cookie to the Cookie header.
    pub fn push<T: Into<String>>(&mut self, name_tref: T, value_tref: T) {
        let name = name_tref.into();
        let value = value_tref.into();
        self.cookies.push((name.clone(), value.clone()));
        if self.index.get(&name) == None {
            self.index.insert(name, value);
        }
    }


    /// Get value of cookie from name. If duplicate names were pushed to the
    /// Cookie header, this function will only return the first one.
    pub fn get<T: Borrow<String>>(&self, name: T) -> Option<String> {
        match self.index.get(name.borrow()) {
            Some(value_ref) => Some((*value_ref).clone()),
            None => None,
        }
    }


    /// Clear the current Cookie, and add one with specified name and value.
    pub fn set<T: Into<String>>(&mut self, name_tref: T, value_tref: T) {
        self.cookies.clear();
        self.index.clear();
        self.push(name_tref, value_tref);
    }

    /// Check if there are any exiting cookie.
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }
}

impl fmt::Display for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        for pair in self.cookies.clone() {
            let cookie = format!("{}={}", pair.0, pair.1);
            if !first {
                try!(f.write_str(" "));
            } else {
                first = false
            }
            try!(f.write_str(&cookie));
            try!(f.write_str(";"));


        }
        Ok(())

    }
}


/*
bench_header!(bench, Cookie, {
    vec![b"foo=bar; baz=quux".to_vec()]
});
*/
