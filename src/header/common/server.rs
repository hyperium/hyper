use header;

/// The `Server` header field.
///
/// They can contain any value, so it just wraps a `String`.
#[derive(Clone, PartialEq, Debug)]
pub struct Server(pub String);

impl_header!(Server,
             "Server",
             String);

bench_header!(bench, Server, { vec![b"Some String".to_vec()] });
