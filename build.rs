extern crate rustc_version;
use rustc_version::{version, Version};

fn main() {
    // Check for a minimum version to see if new rust features can be used
    let version = version().unwrap();
    if version >= Version::parse("1.26.0").unwrap() {
        println!("cargo:rustc-cfg=__hyper_impl_trait_available");
    }
    if version >= Version::parse("1.23.0").unwrap() {
        println!("cargo:rustc-cfg=__hyper_inherent_ascii");
    }
}
