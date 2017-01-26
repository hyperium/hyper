extern crate rustc_version as rustc;

fn main() {
    if rustc::version_matches(">= 1.9") {
        println!("cargo:rustc-cfg=has_deprecated");
    }
}
