/// The `User-Agent` header field.
///
/// They can contain any value, so it just wraps a `String`.
#[derive(Clone, PartialEq, Debug)]
pub struct UserAgent(pub String);

impl_header!(UserAgent,
             "User-Agent",
             String);

bench_header!(bench, UserAgent, { vec![b"cargo bench".to_vec()] });

#[test] fn test_format() {
    use std::borrow::ToOwned;
    use header::Headers;
    let mut head = Headers::new();
    head.set(UserAgent("Bunnies".to_owned()));
    assert!(head.to_string() == "User-Agent: Bunnies\r\n".to_owned());
}
