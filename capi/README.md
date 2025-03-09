# C API for hyper

This provides auxiliary pieces for a C API to use the hyper library.

## Unstable

The C API of hyper is currently **unstable**, which means it's not part of the semver contract as the rest of the Rust API is.

Because of that, it's only accessible if `--cfg hyper_unstable_ffi` is passed to `rustc` when compiling. The easiest way to do that is setting the `RUSTFLAGS` environment variable.

## Building

The C API is part of the Rust library, but isn't compiled by default. Using `cargo`, staring with `1.64.0`, it can be compiled with the following command:

```
RUSTFLAGS="--cfg hyper_unstable_ffi" cargo rustc --features client,http1,http2,ffi --crate-type cdylib
```

### (Optional) With `cargo-c`

If using `cargo-c`, you can build and install a shared library with the following command:

```
RUSTFLAGS="--cfg hyper_unstable_ffi" cargo cbuild --features client,http1,http2,ffi --release
```
