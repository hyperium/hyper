macro_rules! cfg_feature {
    (
        #![$meta:meta]
        $($item:item)*
    ) => {
        $(
            #[cfg($meta)]
            #[cfg_attr(docsrs, doc(cfg($meta)))]
            $item
        )*
    }
}

macro_rules! cfg_any_http {
    ($($item:item)*) => {
        cfg_feature! {
            #![any(feature = "http1", feature = "http2")]
            $($item)*
        }
    }
}

cfg_any_http! {
    macro_rules! cfg_http1 {
        ($($item:item)*) => {
            cfg_feature! {
                #![feature = "http1"]
                $($item)*
            }
        }
    }

    macro_rules! cfg_http2 {
        ($($item:item)*) => {
            cfg_feature! {
                #![feature = "http2"]
                $($item)*
            }
        }
    }

    macro_rules! cfg_client {
        ($($item:item)*) => {
            cfg_feature! {
                #![feature = "client"]
                $($item)*
            }
        }
    }
}
