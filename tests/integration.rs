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
            headers: {
                "date" => SOME,
            },
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
    date_isnt_overwritten,
    client:
        request:
            ;
        response:
            status: 200,
            headers: {
                "date" => "let me through",
            },
            ;
    server:
        request:
            ;
        response:
            headers: {
                "date" => "let me through",
            },
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
    get_body_2_keeps_alive,
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
    get_strip_connection_header,
    client:
        request:
            uri: "/",
            ;
        response:
            status: 200,
            headers: {
                // h2 doesn't actually receive the connection header
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
                "connection" => "close",
            },
            body: "hello world",
            ;
}

t! {
    get_strip_keep_alive_header,
    client:
        request:
            uri: "/",
            ;
        response:
            status: 200,
            headers: {
                // h2 doesn't actually receive the keep-alive header
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
                "keep-alive" => "timeout=5, max=1000",
            },
            body: "hello world",
            ;
}

t! {
    get_strip_upgrade_header,
    client:
        request:
            uri: "/",
            ;
        response:
            status: 200,
            headers: {
                // h2 doesn't actually receive the upgrade header
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
                "upgrade" => "h2c",
            },
            body: "hello world",
            ;
}

t! {
    get_allow_te_trailers_header,
    client:
        request:
            uri: "/",
            headers: {
                // http2 strips connection headers other than TE "trailers"
                "te" => "trailers",
            },
            ;
        response:
            status: 200,
            ;
    server:
        request:
            uri: "/",
            headers: {
                "te" => "trailers",
            },
            ;
        response:
            ;
}

#[tokio::test]
async fn h2_strips_te_header_with_repeated_values() {
    use http_body_util::Empty;
    use hyper::body::Bytes;
    use tokio::net::{TcpListener, TcpStream};

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let service = hyper::service::service_fn(|req: hyper::Request<hyper::body::Incoming>| {
            let te_count = req.headers().get_all("te").iter().count();
            async move {
                let mut res = hyper::Response::new(Empty::<Bytes>::new());
                res.headers_mut().insert("x-te-count", te_count.into());
                Ok::<_, hyper::Error>(res)
            }
        });
        hyper::server::conn::http2::Builder::new(TokioExecutor)
            .serve_connection(TokioIo::new(stream), service)
            .await
            .unwrap();
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let (mut client, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor)
        .handshake(TokioIo::new(stream))
        .await
        .unwrap();
    tokio::spawn(conn);

    // Only the first TE value used to be checked, so the second one reached
    // the peer and the request was rejected as malformed.
    let req = hyper::Request::builder()
        .uri(format!("http://{addr}/"))
        .header("te", "trailers")
        .header("te", "gzip")
        .body(Empty::<Bytes>::new())
        .unwrap();
    let res = client.send_request(req).await.unwrap();

    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["x-te-count"], "0");
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
    post_outgoing_length,
    client:
        request:
            method: "POST",
            uri: "/hello",
            body: "hello, world!",
            ;
        response:
            ;
    server:
        request:
            method: "POST",
            uri: "/hello",
            headers: {
                "content-length" => "13",
            },
            body: "hello, world!",
            ;
        response:
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
    http2_parallel_10,
    parallel: 0..10
}
