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

macro_rules! cfg_proto {
    ($($item:item)*) => {
        cfg_feature! {
            #![any(http_client, http_server)]
            $($item)*
        }
    }
}

cfg_proto! {
    macro_rules! cfg_client {
        ($($item:item)*) => {
            cfg_feature! {
                #![client]
                $($item)*
            }
        }
    }

    macro_rules! cfg_server {
        ($($item:item)*) => {
            cfg_feature! {
                #![server]
                $($item)*
            }
        }
    }
}
