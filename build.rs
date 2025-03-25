use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        http1: { feature = "http1" },
        http2: { feature = "http2" },

        client : { feature = "client" },
        server : { feature = "server" },

        ffi: { feature = "ffi" },
        full: { feature = "full" },
        nightly: { feature = "nightly" },
        runtime: { feature = "runtime" },
        tracing: { feature = "tracing" },

        http_client: { all(any(http1, http2), client) },
        http1_client: { all(http1, client) },
        http2_client: { all(http2, client) },

        http_server: { all(any(http1, http2), server) },
        http1_server: { all(http1, server) },
        http2_server: { all(http2, server) },
    }
}
