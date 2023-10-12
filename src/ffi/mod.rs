// We have a lot of c-types in here, stop warning about their names!
#![allow(non_camel_case_types)]
// fmt::Debug isn't helpful on FFI types
#![allow(missing_debug_implementations)]

//! # hyper C API
//!
//! This part of the documentation describes the C API for hyper. That is, how
//! to *use* the hyper library in C code. This is **not** a regular Rust
//! module, and thus it is not accessible in Rust.
//!
//! ## Unstable
//!
//! The C API of hyper is currently **unstable**, which means it's not part of
//! the semver contract as the rest of the Rust API is. Because of that, it's
//! only accessible if `--cfg hyper_unstable_ffi` is passed to `rustc` when
//! compiling. The easiest way to do that is setting the `RUSTFLAGS`
//! environment variable.
//!
//! ## Building
//!
//! The C API is part of the Rust library, but isn't compiled by default. Using
//! `cargo`, staring with `1.64.0`, it can be compiled with the following command:
//!
//! ```notrust
//! RUSTFLAGS="--cfg hyper_unstable_ffi" cargo rustc --crate-type cdylib --features client,http1,http2,ffi
//! ```

#[cfg(not(hyper_unstable_ffi))]
compile_error!(
    "\
    The `ffi` feature is unstable, and requires the \
    `RUSTFLAGS='--cfg hyper_unstable_ffi'` environment variable to be set.\
"
);

#[macro_use]
mod macros;

mod body;
mod client;
mod error;
mod http_types;
mod io;
mod server;
mod task;
mod time;
mod userdata;

pub use self::body::*;
pub use self::client::*;
pub use self::error::*;
pub use self::http_types::*;
pub use self::io::*;
pub use self::server::*;
pub use self::task::*;
pub use self::userdata::hyper_userdata_drop;

/// Return in iter functions to continue iterating.
pub const HYPER_ITER_CONTINUE: libc::c_int = 0;
/// Return in iter functions to stop iterating.
pub const HYPER_ITER_BREAK: libc::c_int = 1;

/// An HTTP Version that is unspecified.
pub const HYPER_HTTP_VERSION_NONE: libc::c_int = 0;
/// The HTTP/1.0 version.
pub const HYPER_HTTP_VERSION_1_0: libc::c_int = 10;
/// The HTTP/1.1 version.
pub const HYPER_HTTP_VERSION_1_1: libc::c_int = 11;
/// The HTTP/2 version.
pub const HYPER_HTTP_VERSION_2: libc::c_int = 20;

/// cbindgen:ignore
static VERSION_CSTR: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

ffi_fn! {
    /// Returns a static ASCII (null terminated) string of the hyper version.
    fn hyper_version() -> *const libc::c_char {
        VERSION_CSTR.as_ptr() as _
    } ?= std::ptr::null()
}
