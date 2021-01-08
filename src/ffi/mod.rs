// We have a lot of c-types in here, stop warning about their names!
#![allow(non_camel_case_types)]

// We may eventually allow the FFI to be enabled without `client` or `http1`,
// that is why we don't auto enable them as `ffi = ["client", "http1"]` in
// the `Cargo.toml`.
//
// But for now, give a clear message that this compile error is expected.
#[cfg(not(all(feature = "client", feature = "http1")))]
compile_error!("The `ffi` feature currently requires the `client` and `http1` features.");

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
mod task;

pub(crate) use self::body::UserBody;
pub(crate) use self::http_types::HeaderCaseMap;

pub const HYPER_ITER_CONTINUE: libc::c_int = 0;
#[allow(unused)]
pub const HYPER_ITER_BREAK: libc::c_int = 1;

pub const HYPER_HTTP_VERSION_NONE: libc::c_int = 0;
pub const HYPER_HTTP_VERSION_1_0: libc::c_int = 10;
pub const HYPER_HTTP_VERSION_1_1: libc::c_int = 11;
pub const HYPER_HTTP_VERSION_2: libc::c_int = 20;

struct UserDataPointer(*mut std::ffi::c_void);

// We don't actually know anything about this pointer, it's up to the user
// to do the right thing.
unsafe impl Send for UserDataPointer {}

/// cbindgen:ignore
static VERSION_CSTR: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

ffi_fn! {
    /// Returns a static ASCII (null terminated) string of the hyper version.
    fn hyper_version() -> *const libc::c_char {
        VERSION_CSTR.as_ptr() as _
    }
}
