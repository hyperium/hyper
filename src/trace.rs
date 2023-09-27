#![allow(unused_macros)]

macro_rules! debug {
    ($($arg:tt)+) => {
        #[cfg(feature = "tracing")]
        {
            println!($($arg)+);
            tracing::debug!($($arg)+);
        }
    }
}

macro_rules! debug_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            let span = tracing::debug_span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! error {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            tracing::error!($($arg)+);
        }
    }
}

macro_rules! error_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            let span = tracing::error_span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            tracing::info!($($arg)+);
        }
    }
}

macro_rules! info_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            let span = tracing::info_span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            tracing::trace!($($arg)+);
        }
    }
}

macro_rules! trace_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            let span = tracing::trace_span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            let span = tracing::span!($($arg)+);
            let _ = span.enter();
        }
    }
}

macro_rules! warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            tracing::warn!($($arg)+);
        }
    }
}

macro_rules! warn_span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        {
            let span = tracing::warn_span!($($arg)+);
            let _ = span.enter();
        }
    }
}
