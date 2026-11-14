# Testing hyper

Hyper's tests live next to the code they exercise, in `#[cfg(test)]` modules, and in the integration test files under `tests/`. Most behaviour is gated behind Cargo features, so run tests with the features that enable the code path you changed.

## Quick start

```bash
cargo test --features full
```

`full` enables the `client`, `http1`, `http2`, and `server` features. Passing it is the fastest way to match what CI runs on a normal pull request.

## Testing a single feature

If you are working on one area, a narrower feature set compiles faster:

```bash
# HTTP/1 only
cargo test --features http1

# Client and HTTP/2
cargo test --features client,http2

# Server and HTTP/1
cargo test --features server,http1
```

Some tests are only compiled when their feature is enabled, so a passing `cargo test --features http1` does not tell you anything about the client or HTTP/2 code.

## Running a single test

Use the test name as a filter:

```bash
cargo test --features full --test integration client_get
```

To run one module or one function inside a test file:

```bash
cargo test --features full some_module::
cargo test --features full some_module::test_name
```

## Benches

The nightly job also compiles and runs the benchmarks:

```bash
cargo test --benches --features full,nightly
```

## Miri

Miri runs parts of the test suite under the Miri interpreter. It is slower and requires the nightly toolchain:

```bash
MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --features http1,http2,client,server,nightly
```

## FFI

The C API has its own tests. They are part of the library target and need the `ffi` feature:

```bash
cargo test --features client,http1,http2,ffi --lib
```

## Style and lints

CI checks formatting and Clippy separately:

```bash
rustfmt --check --edition 2021 $(git ls-files '*.rs')
cargo clippy --features full -- -D warnings
```

To fix formatting locally:

```bash
rustfmt --edition 2021 $(git ls-files '*.rs')
```

## CI matrix

Pull requests run tests on stable, beta, and nightly Rust, on Linux, macOS, and Windows:

- stable and beta: `cargo test --features full`
- nightly: `cargo test --features full,nightly` plus `cargo test --benches --features full,nightly`

If a failure only shows up on one platform or one toolchain, reproduce the job locally with that toolchain and feature set:

```bash
cargo +nightly test --features full,nightly
```
