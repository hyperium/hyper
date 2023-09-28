#![allow(unused_macros)]

macro_rules! debug {
    ($($arg:tt)+) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            tracing::debug!($($arg)+);
        }
    }
}

macro_rules! debug_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            let span = tracing::debug_span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            tracing::error!($($arg)+);
        }
    }
}

macro_rules! error_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            let span = tracing::error_span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            tracing::info!($($arg)+);
        }
    }
}

macro_rules! info_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            let span = tracing::info_span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            tracing::trace!($($arg)+);
        }
    }
}

macro_rules! trace_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            let span = tracing::trace_span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            let span = tracing::span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            tracing::warn!($($arg)+);
        }
    }
}

macro_rules! warn_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            #[cfg(not(hyper_unstable_tracing))]
            compile_error!(
                "\
                The `tracing` feature is unstable, and requires the \
                `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
            "
            );
            let span = tracing::warn_span!($($arg)+);
            let _ = span.enter();
        }
    }
}
