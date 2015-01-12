use header;
use std::fmt::{self, Show};
use header::shared::util::from_one_raw_str;

/// The `Server` header field.
///
/// They can contain any value, so it just wraps a `String`.
#[derive(Clone, PartialEq, Show)]
pub struct Server(pub String);

impl_header!(Server,
             "Server",
             String);

bench_header!(bench, Server, { vec![b"Some String".to_vec()] });
