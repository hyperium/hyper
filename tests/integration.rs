#![deny(warnings)]
#[macro_use]
mod support;
use self::support::*;

t! {
    get_1,
    client:
        request:
            uri: "/",
            ;
        response:
            status: 200,
            ;
    server:
        request:
            uri: "/",
            ;
        response:
            ;
}

t! {
    get_implicit_path,
    client:
        request:
            uri: "",
            ;
        response:
            status: 200,
            ;
    server:
        request:
            uri: "/",
            ;
        response:
            ;
}

t! {
    get_body,
    client:
        request:
            uri: "/",
            ;
        response:
            status: 200,
            headers: {
                "content-length" => 11,
            },
            body: "hello world",
            ;
    server:
        request:
            uri: "/",
            ;
        response:
            headers: {
                "content-length" => 11,
            },
            body: "hello world",
            ;
}

t! {
    get_body_chunked,
    client:
        request:
            uri: "/",
            ;
        response:
            status: 200,
            headers: {
                // h2 doesn't actually receive the transfer-encoding header
            },
            body: "hello world",
            ;
    server:
        request:
            uri: "/",
            ;
        response:
            headers: {
                // http2 should strip this header
                "transfer-encoding" => "chunked",
            },
            body: "hello world",
            ;
}

t! {
    post_chunked,
    client:
        request:
            method: "POST",
            uri: "/post_chunked",
            headers: {
                // http2 should strip this header
                "transfer-encoding" => "chunked",
            },
            body: "hello world",
            ;
        response:
            ;
    server:
        request:
            method: "POST",
            uri: "/post_chunked",
            body: "hello world",
            ;
        response:
            ;
}

t! {
    get_2,
    client:
        request:
            uri: "/1",
            ;
        response:
            status: 200,
            ;
        request:
            uri: "/2",
            ;
        response:
            status: 200,
            ;
    server:
        request:
            uri: "/1",
            ;
        response:
            ;
        request:
            uri: "/2",
            ;
        response:
            ;
}

t! {
    get_parallel_http2,
    parallel: 0..10
}

