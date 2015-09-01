#![deny(warnings)]

#[macro_use]
// TODO: only import header!, blocked by https://github.com/rust-lang/rust/issues/25003
extern crate hyper;

#[cfg(feature = "serde-serialization")]
extern crate serde;

// A header in the form of `X-Foo: some random string`
header! {
    (Foo, "X-Foo") => [String]
}

fn main() {
}
