// For completeness, wrappers around all of tracing's public logging and span macros are provided,
// even if they are not used at the present time.
#![allow(unused_macros)]

#[cfg(all(not(hyper_unstable_tracing), tracing))]
compile_error!(
    "\
    The `tracing` feature is unstable, and requires the \
    `RUSTFLAGS='--cfg hyper_unstable_tracing'` environment variable to be set.\
"
);

macro_rules! debug {
    ($($arg:tt)+) => {
        #[cfg(tracing)]
        {
            tracing::debug!($($arg)+);
        }
    }
}

macro_rules! debug_span {
    ($($arg:tt)*) => {
        {
            #[cfg(tracing)]
            {
                let _span = tracing::debug_span!($($arg)+);
                _span.entered()
            }
        }
    }
}

macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(tracing)]
        {
            tracing::error!($($arg)+);
        }
    }
}

macro_rules! error_span {
    ($($arg:tt)*) => {
        {
            #[cfg(tracing)]
            {
                let _span = tracing::error_span!($($arg)+);
                _span.entered()
            }
        }
    }
}

macro_rules! info {
    ($($arg:tt)*) => {
        #[cfg(tracing)]
        {
            tracing::info!($($arg)+);
        }
    }
}

macro_rules! info_span {
    ($($arg:tt)*) => {
        {
            #[cfg(tracing)]
            {
                let _span = tracing::info_span!($($arg)+);
                _span.entered()
            }
        }
    }
}

macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(tracing)]
        {
            tracing::trace!($($arg)+);
        }
    }
}

macro_rules! trace_span {
    ($($arg:tt)*) => {
        {
            #[cfg(tracing)]
            {
                let _span = tracing::trace_span!($($arg)+);
                _span.entered()
            }
        }
    }
}

macro_rules! span {
    ($($arg:tt)*) => {
        {
            #[cfg(tracing)]
            {
                let _span = tracing::span!($($arg)+);
                _span.entered()
            }
        }
    }
}

macro_rules! warn {
    ($($arg:tt)*) => {
        #[cfg(tracing)]
        {
            tracing::warn!($($arg)+);
        }
    }
}

macro_rules! warn_span {
    ($($arg:tt)*) => {
        {
            #[cfg(tracing)]
            {
                let _span = tracing::warn_span!($($arg)+);
                _span.entered()
            }
        }
    }
}
