macro_rules! cfg_any_http {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                feature = "http1",
                feature = "http2",
            ))]
            #[cfg_attr(docsrs, doc(cfg(any(
                feature = "http1",
                feature = "http2",
            ))))]
            $item
        )*
    }
}

cfg_any_http! {
    macro_rules! cfg_http1 {
        ($($item:item)*) => {
            $(
                #[cfg(feature = "http1")]
                #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
                $item
            )*
        }
    }

    macro_rules! cfg_http2 {
        ($($item:item)*) => {
            $(
                #[cfg(feature = "http2")]
                #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
                $item
            )*
        }
    }
}
