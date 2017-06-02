extern crate version_check as rustc;

fn main() {
    if !rustc::is_min_version("1.9.0").unwrap_or(true) {
        println!("cargo:rustc-cfg=has_deprecated");
    }
}
