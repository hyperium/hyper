// For completeness, wrappers around all of tracing's public logging and span macros are provided, 
// even if they are not used at the present time.
#![allow(unused_macros)]

//! Internal Tracing macro module
//!
//! The [`trace`][crate::trace] module is an internal module that contains wrapper macros encapsulating
//! [`tracing`][tracing]'s macros. These macros allow for conditional expansion of
//! [`tracing`][tracing] macros during compilation when the `tracing` feature is enabled, or trimming
//! them when the feature is disabled, all in a concise manner.
//!
//! The macros from the [`trace`][crate::trace] module are declared by default and can be used throughout the
//! crate. However, as the contents of these macros are conditionally compiled, they are effectively trimmed
//! during inline expansion when the `tracing` feature is disabled.
//!
//! # Unstable
//!
//! The [`tracing`][tracing] module is currenty **unstable**, hence the existence of this module. As a
//! result, hyper's [`tracing`][tracing] logs are only accessibe if `--cfg hyper_unstable_tracing` is
//! passed to `rustc` when compiling. The easiest way to do that is through setting the `RUSTFLAGS`
//! enviornment variable.
//!
//! # Building
//!  
//! Enabling [`trace`][crate::trace] logs, can be done with the following `cargo` command, as of
//! version `1.64.0`:
//!
//! ```notrust
//! RUSTFLAGS="--cfg hyper_unstable_tracing" cargo rustc --features client,http1,http2,tracing --crate-type cdylib
//! ```

#[cfg(not(hyper_unstable_tracing))]
compile_error!(
    "\
    The `tracing` feature is unstable, and requires the \
    `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
    "
    );

macro_rules! debug {
    ($($arg:tt)+) => {
        #[cfg(feature = "tracing")]
        tracing::debug!($($arg)+);
    }
}

macro_rules! debug_span {
    ($($arg:tt)*) => {
        {
            #[cfg(feature = "tracing")]
            {
                let _span = tracing::debug_span!($($arg)+);
                _span.entered();
            }
        }
    }
}

macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::error!($($arg)+);
    }
}

macro_rules! error_span {
    ($($arg:tt)*) => {
        {
            #[cfg(feature = "tracing")]
            {
                let _span = tracing::error_span!($($arg)+);
                _span.entered();
            }
        }
    }
}

macro_rules! info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::info!($($arg)+);
    }
}

macro_rules! info_span {
    ($($arg:tt)*) => {
        {
            #[cfg(feature = "tracing")]
            {
                let _span = tracing::info_span!($($arg)+);
                _span.entered();
            }
        }
    }
}

macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::trace!($($arg)+);
    }
}

macro_rules! trace_span {
    ($($arg:tt)*) => {
        {
            #[cfg(feature = "tracing")]
            {
                let _span = tracing::trace_span!($($arg)+);
                _span.entered();
            }
        }
    }
}

macro_rules! span {
    ($($arg:tt)*) => {
        {
            #[cfg(feature = "tracing")]
            {
                let _span = tracing::span!($($arg)+);
                _span.entered();
            }
        }
    }
}

macro_rules! warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::warn!($($arg)+);
    }
}

macro_rules! warn_span {
    ($($arg:tt)*) => {
        {
            #[cfg(feature = "tracing")]
            {
                let _span = tracing::warn_span!($($arg)+);
                _span.entered();
            }
        }
    }
}
