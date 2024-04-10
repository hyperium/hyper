use futures_util::future::join_all;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[path = "../benches/support/mod.rs"]
mod support;
use support::TokioIo;

pub mod helpers {
    use bytes::Bytes;
    use http_body_util::combinators::BoxBody;
    use http_body_util::{BodyExt, Empty, Full};

    pub fn host_addr(uri: &http::Uri) -> Option<String> {
        uri.authority().and_then(|auth| Some(auth.to_string()))
    }

    pub fn empty() -> BoxBody<Bytes, hyper::Error> {
        Empty::<Bytes>::new()
            .map_err(|never| match never {})
            .boxed()
    }

    pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
        Full::new(chunk.into())
            .map_err(|never| match never {})
            .boxed()
    }
}

pub mod proxy_endpoint {
    use super::helpers::{empty, full, host_addr};
    use super::TokioIo;
    use bytes::Bytes;
    use http::header;
    use http_body_util::{combinators::BoxBody, BodyExt};
    use hyper::client::conn::http1::Builder;
    use hyper::server::conn::http1;
    use hyper::service::service_fn;
    use hyper::upgrade::Upgraded;
    use hyper::{http, Method, Request, Response};
    use std::net::SocketAddr;
    use std::str::FromStr;
    use tokio::net::TcpStream;

    pub async fn proxy_endpoint_main() -> Result<(), Box<dyn std::error::Error>> {
        let addr = SocketAddr::from_str(format!("{}:{}", "127.0.0.1", "5000").as_str())
            .expect("Failed to parse address");
        while let Ok(stream) = TcpStream::connect(addr).await {
            println!("Connected to {}", addr);
            let (mut send_request, conn) = Builder::new().handshake(TokioIo::new(stream)).await?;
            tokio::spawn(conn.with_upgrades());
            let req = Request::builder()
                .method(Method::CONNECT)
                .uri(addr.to_string())
                .header(header::UPGRADE, "")
                .header("custom-header", "")
                .body(empty())
                .unwrap();
            let res = send_request.send_request(req).await?;
            let stream = hyper::upgrade::on(res).await?;

            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(stream, service_fn(proxy))
                .with_upgrades()
                .await
            {
                println!("Failed to serve connection: {:?}", err);
            }
        }
        Ok(())
    }

    async fn proxy(
        req: Request<hyper::body::Incoming>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        println!("req: {:?}", req);

        if Method::CONNECT == req.method() {
            if let Some(addr) = host_addr(req.uri()) {
                tokio::task::spawn(async move {
                    match hyper::upgrade::on(req).await {
                        Ok(upgraded) => {
                            if let Err(e) = tunnel(upgraded, addr).await {
                                println!("server io error: {}", e);
                            };
                        }
                        Err(e) => println!("upgrade error: {}", e),
                    }
                });

                Ok(Response::new(empty()))
            } else {
                println!("CONNECT host is not socket addr: {:?}", req.uri());
                let mut resp = Response::new(full("CONNECT must be to a socket address"));
                *resp.status_mut() = http::StatusCode::BAD_REQUEST;

                Ok(resp)
            }
        } else {
            let host = req.uri().host().expect("uri has no host");
            let port = req.uri().port_u16().unwrap_or(80);

            let stream = TcpStream::connect((host, port)).await.unwrap();
            let io = TokioIo::new(stream);

            let (mut sender, conn) = Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .handshake(io)
                .await?;
            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    println!("Connection failed: {:?}", err);
                }
            });

            let resp = sender.send_request(req).await?;
            Ok(resp.map(|b| b.boxed()))
        }
    }

    async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
        let mut server = TcpStream::connect(addr.clone()).await?;
        let mut upgraded = TokioIo::new(upgraded);
        let (from_client, from_server) =
            tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;
        println!(
            "proxy_endpoint => from_client = {} | from_server = {}",
            from_client, from_server
        );
        Ok(())
    }
}

pub mod proxy_master {
    pub mod proxy_pool {
        use hyper::upgrade::Upgraded;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        #[derive(Debug, Clone, Default)]
        pub struct ProxyPool {
            pool: Arc<Mutex<Vec<Upgraded>>>,
        }

        impl ProxyPool {
            pub async fn put(&self, stream: Upgraded) {
                self.pool.lock().await.push(stream);
            }

            pub async fn get(&self) -> Option<Upgraded> {
                let mut lock = self.pool.lock().await;

                // We have all proxy connection now, so we can pick any of them by arbitrary condition

                // Just pop the last one for example
                lock.pop()
            }
        }
    }

    pub mod proxy_endpoint {
        use super::super::helpers::empty;
        use super::super::TokioIo;
        use super::proxy_pool::ProxyPool;
        use bytes::Bytes;
        use http_body_util::combinators::BoxBody;
        use hyper::server;
        use hyper::service::service_fn;
        use hyper::{Method, Request, Response};
        use tokio::net::TcpListener;

        pub async fn listen_for_proxies_connecting(
            pool: ProxyPool,
            proxy_listener: TcpListener,
        ) -> () {
            while let Ok((stream, addr)) = proxy_listener.accept().await {
                let pool = pool.clone();
                tokio::spawn(async move {
                    if let Err(err) = server::conn::http1::Builder::new()
                        .preserve_header_case(true)
                        .title_case_headers(true)
                        .serve_connection(
                            TokioIo::new(stream),
                            service_fn(move |req| handle_proxy_request(pool.clone(), req)),
                        )
                        .with_upgrades()
                        .await
                    {
                        println!("Failed to serve connection from addr {:?}: {:?}", addr, err);
                    }
                });
            }
        }

        async fn handle_proxy_request(
            pool: ProxyPool,
            req: Request<hyper::body::Incoming>,
        ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
            if Method::CONNECT == req.method() {
                // Received an HTTP request like:
                // ```
                // CONNECT www.domain.com:443 HTTP/1.1
                // Host: www.domain.com:443
                // Proxy-Connection: Keep-Alive
                // ```
                //
                // When HTTP method is CONNECT we should return an empty body
                // then we can eventually upgrade the connection and talk a new protocol.
                //
                // Note: only after client received an empty body with STATUS_OK can the
                // connection be upgraded, so we can't return a response inside
                // `on_upgrade` future.
                tokio::spawn(async move {
                    match hyper::upgrade::on(req).await {
                        Ok(upgraded) => {
                            // We can put proxy along with req here
                            pool.put(upgraded).await;
                        }
                        Err(e) => println!("upgrade error: {}", e),
                    }
                });
                Ok(Response::new(empty()))
            } else {
                // TODO : Process request - can register proxy here
                println!("NOT CONNECT request");
                Ok(Response::new(empty()))
            }
        }
    }

    pub mod clients_endpoint {
        use super::super::helpers::empty;
        use super::super::TokioIo;
        use super::proxy_pool::ProxyPool;
        use bytes::Bytes;
        use http_body_util::combinators::BoxBody;
        use hyper::service::service_fn;
        use hyper::{client, server, Method, Request, Response};
        use tokio::io::copy_bidirectional;
        use tokio::net::TcpListener;

        pub async fn listen_for_clients_connecting(pool: ProxyPool, client_listener: TcpListener) {
            while let Ok((stream, addr)) = client_listener.accept().await {
                let pool = pool.clone();
                tokio::spawn(async move {
                    if let Err(err) = server::conn::http1::Builder::new()
                        .preserve_header_case(true)
                        .title_case_headers(true)
                        .serve_connection(
                            TokioIo::new(stream),
                            service_fn(move |req| handle_client_request(pool.clone(), req)),
                        )
                        .with_upgrades()
                        .await
                    {
                        println!("Failed to serve connection from addr {:?}: {:?}", addr, err);
                    }
                });
            }
        }

        async fn handle_client_request(
            pool: ProxyPool,
            mut req: Request<hyper::body::Incoming>,
        ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
            if Method::CONNECT == req.method() {
                tokio::spawn(async move {
                    match hyper::upgrade::on(&mut req).await {
                        Ok(upgraded) => {
                            let proxy = pool.get().await.unwrap();
                            let (mut send_request, conn) =
                                client::conn::http1::Builder::new().handshake(proxy).await?;
                            tokio::spawn(conn.with_upgrades());
                            let res = send_request.send_request(req).await?;
                            let stream = hyper::upgrade::on(res).await?;
                            let (from_client, from_server) = copy_bidirectional(
                                &mut TokioIo::new(upgraded),
                                &mut TokioIo::new(stream),
                            )
                            .await
                            .unwrap();
                            println!(
                                "proxy_master => from_client = {} | from_server = {}",
                                from_client, from_server
                            );
                        }
                        Err(e) => println!("upgrade error = {}", e),
                    }
                    Ok::<(), hyper::Error>(())
                });
                Ok(Response::new(empty()))
            } else {
                Ok(Response::new(empty()))
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let pool = proxy_master::proxy_pool::ProxyPool::default();
    let addr_proxies = SocketAddr::from(([127, 0, 0, 1], 5000));
    let proxy_listener = TcpListener::bind(addr_proxies).await.unwrap();
    println!("Listening on for proxies on: {}", addr_proxies);
    let addr_clients = SocketAddr::from(([127, 0, 0, 1], 4000));
    let client_listener = TcpListener::bind(addr_clients).await.unwrap();
    println!("Listening on for clients on: {}", addr_clients);

    let proxy_listener_pool = pool.clone();

    let proxy_endpoint_main_task = tokio::task::spawn(async move {
        proxy_endpoint::proxy_endpoint_main().await.unwrap();
    });

    let proxy_listener_task = tokio::task::spawn(async move {
        proxy_master::proxy_endpoint::listen_for_proxies_connecting(
            proxy_listener_pool,
            proxy_listener,
        )
        .await
    });
    let proxy_listener_pool = pool.clone();
    let clients_listener_task = tokio::task::spawn(async move {
        proxy_master::clients_endpoint::listen_for_clients_connecting(
            proxy_listener_pool,
            client_listener,
        )
        .await;
    });
    let _ = join_all(vec![
        proxy_listener_task,
        clients_listener_task,
        proxy_endpoint_main_task,
    ])
    .await;
}
