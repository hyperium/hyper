fn main() {
    // Add to the list of expected config names and values that is used when checking the reachable
    // cfg expressions with the unexpected_cfgs lint.
    //
    // See https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-check-cfg
    println!("cargo:rustc-check-cfg=cfg(http1)");
    println!("cargo:rustc-check-cfg=cfg(http2)");
    println!("cargo:rustc-check-cfg=cfg(client)");
    println!("cargo:rustc-check-cfg=cfg(server)");
    println!("cargo:rustc-check-cfg=cfg(ffi)");
    println!("cargo:rustc-check-cfg=cfg(full)");
    println!("cargo:rustc-check-cfg=cfg(nightly)");
    println!("cargo:rustc-check-cfg=cfg(runtime)"); // TODO evaluate if this is needed (see below)
    println!("cargo:rustc-check-cfg=cfg(tracing)");
    println!("cargo:rustc-check-cfg=cfg(http_client)");
    println!("cargo:rustc-check-cfg=cfg(http1_client)");
    println!("cargo:rustc-check-cfg=cfg(http2_client)");
    println!("cargo:rustc-check-cfg=cfg(http_server)");
    println!("cargo:rustc-check-cfg=cfg(http1_server)");
    println!("cargo:rustc-check-cfg=cfg(http2_server)");

    // Add cfg flags that simplify using cfg expressions in the code. e.g. instead of
    // `#[cfg(all(any(feature = "http1", feature = "http2"), feature = "server")]` you can use
    // `#[cfg(http_server)]`
    //
    // See https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-cfg
    #[cfg(feature = "http1")]
    println!("cargo:rustc-cfg=http1");

    #[cfg(feature = "http2")]
    println!("cargo:rustc-cfg=http2");

    #[cfg(any(feature = "http1", feature = "http2"))]
    println!("cargo:rustc-cfg=http");

    #[cfg(feature = "client")]
    println!("cargo:rustc-cfg=client");

    #[cfg(feature = "server")]
    println!("cargo:rustc-cfg=server");

    #[cfg(feature = "ffi")]
    println!("cargo:rustc-cfg=ffi");

    #[cfg(feature = "full")]
    println!("cargo:rustc-cfg=full");

    #[cfg(feature = "nightly")]
    println!("cargo:rustc-cfg=nightly");

    // TODO: this feature doesn't actually exist in the cargo.toml
    // this condition was added to simplify the conditions in src/mock.rs, but I'm not sure if those
    // conditions were actually working as intended
    // #[cfg(feature = "runtime")]
    // println!("cargo:rustc-cfg=runtime");

    #[cfg(feature = "tracing")]
    println!("cargo:rustc-cfg=tracing");

    #[cfg(all(any(feature = "http1", feature = "http2"), feature = "client"))]
    println!("cargo:rustc-cfg=http_client");

    #[cfg(all(feature = "http1", feature = "client"))]
    println!("cargo:rustc-cfg=http1_client");

    #[cfg(all(feature = "http2", feature = "client"))]
    println!("cargo:rustc-cfg=http2_client");

    #[cfg(all(any(feature = "http1", feature = "http2"), feature = "server"))]
    println!("cargo:rustc-cfg=http_server");

    #[cfg(all(feature = "http1", feature = "server"))]
    println!("cargo:rustc-cfg=http1_server");

    #[cfg(all(feature = "http2", feature = "server"))]
    println!("cargo:rustc-cfg=http2_server");
}
