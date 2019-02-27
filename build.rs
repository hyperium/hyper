extern crate rustc_version;

use rustc_version::{version, Version};

fn main() {
    if version().unwrap() >= Version::parse("1.30.0").unwrap() {
        println!("cargo:rustc-cfg=error_source");
    }
}
