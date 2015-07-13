use std::fmt::{self, Display};
use std::str::FromStr;

header! {
    #[doc="`Content-Range` header, defined in"]
    #[doc="[RFC7233](http://tools.ietf.org/html/rfc7233#section-4.2)"]
    (ContentRange, "Content-Range") => [ContentRangeSpec]

    test_range {
        test_header!(test1, vec![b"bytes 0-499/500"],
            Some(ContentRange(ContentRangeSpec {
                range: Some((0, 499)),
                instance_length: Some(500)
            })));
        test_header!(test2, vec![b"bytes 0-499/*"],
            Some(ContentRange(ContentRangeSpec {
                range: Some((0, 499)),
                instance_length: None
            })));
        test_header!(test3, vec![b"bytes */500"],
            Some(ContentRange(ContentRangeSpec {
                range: None,
                instance_length: Some(500)
            })));
        test_header!(test4, vec![b"bytes 0-499"], None::<ContentRange>);
        test_header!(test5, vec![b"bytes"], None::<ContentRange>);
        test_header!(test6, vec![b"bytes 499-0/500"], None::<ContentRange>);
        test_header!(test7, vec![b""], None::<ContentRange>);
    }
}


/// Content Range, described in [RFC7233](https://tools.ietf.org/html/rfc7233#section-4.2)
///
/// # ABNF
/// ```plain
/// Range = "Content-Range" ":" content-range-spec
/// content-range-spec      = byte-content-range-spec
/// byte-content-range-spec = bytes-unit SP
///                           byte-range-resp-spec "/"
///                           ( instance-length | "*" )
/// byte-range-resp-spec = (first-byte-pos "-" last-byte-pos)
///                                | "*"
/// instance-length           = 1*DIGIT
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRangeSpec {
    /// First and last bytes of the range
    pub range: Option<(u64, u64)>,

    /// Total length of the instance, can be omitted if unknown
    pub instance_length: Option<u64>,
}

impl FromStr for ContentRangeSpec {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        let prefix = "bytes ";
        if !s.starts_with(prefix) {
            return Err(());
        }
        let s = &s[prefix.len()..];

        let parts = s.split('/').collect::<Vec<_>>();
        if parts.len() != 2 {
            return Err(());
        }

        let instance_length = if parts[1] == "*" {
            None
        } else {
            Some(try!(parts[1].parse().map_err(|_| ())))
        };

        let range = if parts[0] == "*" {
            None
        } else {
            let range = parts[0].split('-').collect::<Vec<_>>();
            if range.len() != 2 {
                return Err(());
            }
            let first_byte = try!(range[0].parse().map_err(|_| ()));
            let last_byte = try!(range[1].parse().map_err(|_| ()));
            if last_byte < first_byte {
                return Err(());
            }
            Some((first_byte, last_byte))
        };

        Ok(ContentRangeSpec {
            range: range,
            instance_length: instance_length
        })
    }
}

impl Display for ContentRangeSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
	try!(f.write_str("bytes "));
        match self.range {
            Some((first_byte, last_byte)) => {
                try!(f.write_fmt(format_args!("{}-{}", first_byte, last_byte)));
            },
            None => {
                try!(f.write_str("*"));
            }
        };
	try!(f.write_str("/"));
	if let Some(v) = self.instance_length {
	    f.write_fmt(format_args!("{}", v))
	} else {
	    f.write_str("*")
	}
    }
}
