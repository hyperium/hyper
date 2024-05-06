fn main() {
    println!("cargo:rustc-check-cfg=cfg(hyper_unstable_ffi)");
    println!("cargo:rustc-check-cfg=cfg(hyper_unstable_tracing)");
}
