macro_rules! cfg_http2 {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "http2")]
            //#[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
            $item
        )*
    }
}
