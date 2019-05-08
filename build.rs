extern crate rustc_version;

use rustc_version::{version, Version};

fn main() {
    let version = version().unwrap();
    if version >= Version::parse("1.30.0").unwrap() {
        println!("cargo:rustc-cfg=error_source");
    }
    if version >= Version::parse("1.34.0").unwrap() {
        println!("cargo:rustc-cfg=try_from");
    }
}
