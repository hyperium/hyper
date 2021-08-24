### v0.14.12 (2021-08-24)


#### Bug Fixes

* **ffi:** on_informational callback had no headers ([39b6d01a](https://github.com/hyperium/hyper/commit/39b6d01aa0e520077bb25e16811f5ece00a224d6))
* **http1:** apply header title case for consecutive dashes (#2613) ([684f2fa7](https://github.com/hyperium/hyper/commit/684f2fa76d44fa2b1b063ad0443a1b0d16dfad0e))
* **http2:** improve errors emitted by HTTP2 `Upgraded` stream shutdown (#2622) ([be08648e](https://github.com/hyperium/hyper/commit/be08648e8298cdb13e9879ee761a73f827268962))


#### Features

* **client:** expose http09 and http1 options on `client::conn::Builder` (#2611) ([73bff4e9](https://github.com/hyperium/hyper/commit/73bff4e98c372ce04b006370c0b0d2af29ea8718), closes [#2461](https://github.com/hyperium/hyper/issues/2461))


### v0.14.11 (2021-07-21)


#### Bug Fixes

* **client:** retry when pool checkout returns closed HTTP2 connection (#2585) ([52214f39](https://github.com/hyperium/hyper/commit/52214f391c0a18dc66d1ccff9c0c004c5da85002))
* **http2:**
  * improve I/O errors emitted by H2Upgraded (#2598) ([f51c677d](https://github.com/hyperium/hyper/commit/f51c677dec9debf60cb336dc938bae103adf17a0))
  * preserve `proxy-authenticate` and `proxy-authorization` headers (#2597) ([52435701](https://github.com/hyperium/hyper/commit/5243570137ae49628cb387fff5611eea0add33bf))


#### Features

* **ffi:** add hyper_request_on_informational ([25d18c0b](https://github.com/hyperium/hyper/commit/25d18c0b74ccf9e51f986daa3b2b98c0109f827a))


### v0.14.10 (2021-07-07)


#### Bug Fixes

* **http1:**
  * reject content-lengths that have a plus sign prefix ([06335158](https://github.com/hyperium/hyper/commit/06335158ca48724db9bf074398067d2db08613e7))
  * protect against overflow in chunked decoder ([efd9a982](https://github.com/hyperium/hyper/commit/efd9a9821fd2f1ae04b545094de76a435b62e70f))


#### Features

* **ffi:** add option to get raw headers from response ([8c89a8c1](https://github.com/hyperium/hyper/commit/8c89a8c1665b6fbec3f13b8c0e84c79464179c89))


### v0.14.9 (2021-06-07)


#### Bug Fixes

* **http1:** reduce memory used with flatten write strategy ([eb0c6463](https://github.com/hyperium/hyper/commit/eb0c64639503bbd4f6e3b1ce3a02bff8eeea7ee8))


### v0.14.8 (2021-05-25)


#### Features

* **client:** allow to config http2 max concurrent reset streams (#2535) ([b9916c41](https://github.com/hyperium/hyper/commit/b9916c410182c6225e857f0cded355ea1b74c865))
* **error:** add `Error::is_parse_too_large` and `Error::is_parse_status` methods (#2538) ([960a69a5](https://github.com/hyperium/hyper/commit/960a69a5878ede82c56f50ac1444a9e75e885a8f))
* **http2:**
  * Implement Client and Server CONNECT support over HTTP/2 (#2523) ([5442b6fa](https://github.com/hyperium/hyper/commit/5442b6faddaff9aeb7c073031a3b7aa4497fda4d), closes [#2508](https://github.com/hyperium/hyper/issues/2508))
  * allow HTTP/2 requests by ALPN when http2_only is unset (#2527) ([be9677a1](https://github.com/hyperium/hyper/commit/be9677a1e782d33c4402772e0fc4ef0a4c49d507))


#### Performance

* **http2:** reduce amount of adaptive window pings as BDP stabilizes (#2550) ([4cd06bf2](https://github.com/hyperium/hyper/commit/4cd06bf2))


### v0.14.7 (2021-04-22)


#### Bug Fixes

* **http1:** http1_title_case_headers should move Builder ([a303b3c3](https://github.com/hyperium/hyper/commit/a303b3c329e6b8ecfa1da0b9b9e94736628167e0))


#### Features

* **server:** implement forgotten settings for case preserving ([4fd6c4cb](https://github.com/hyperium/hyper/commit/4fd6c4cb0b58bb0831ae0f876d858aba1588d0e3))


### v0.14.6 (2021-04-21)


#### Features

* **client:** add option to allow misplaced spaces in HTTP/1 responses (#2506) ([11345394](https://github.com/hyperium/hyper/commit/11345394d968d4817e1a0ee2550228ac0ae7ce74))
* **http1:** add options to preserve header casing (#2480) ([dbea7716](https://github.com/hyperium/hyper/commit/dbea7716f157896bf7d2d417be7b4e382e7dc34f), closes [#2313](https://github.com/hyperium/hyper/issues/2313))


### v0.14.5 (2021-03-26)


#### Bug Fixes

* **client:** omit default port from automatic Host headers (#2441) ([0b11eee9](https://github.com/hyperium/hyper/commit/0b11eee9bde421cdc1680cadabfd38c5aff8e62f))
* **headers:** Support multiple Content-Length values on same line (#2471) ([48fdaf16](https://github.com/hyperium/hyper/commit/48fdaf160689f333e9bb63388d0b1d0fa29a1391), closes [#2470](https://github.com/hyperium/hyper/issues/2470))
* **server:** skip automatic Content-Length headers when not allowed (#2216) ([8cbf9527](https://github.com/hyperium/hyper/commit/8cbf9527dfb313b3f84fcd83260c5c72ce4a1beb), closes [#2215](https://github.com/hyperium/hyper/issues/2215))


#### Features

* **client:** allow HTTP/0.9 responses behind a flag (#2473) ([68d4e4a3](https://github.com/hyperium/hyper/commit/68d4e4a3db91fb43f41a8c4fce1175ddb56816af), closes [#2468](https://github.com/hyperium/hyper/issues/2468))
* **server:** add `AddrIncoming::from_listener` constructor (#2439) ([4c946af4](https://github.com/hyperium/hyper/commit/4c946af49cc7fbbc6bd4894283a654625c2ea383))


### v0.14.4 (2021-02-05)


#### Bug Fixes

* **build**: Fix compile error when only `http1` feature was enabled.


### v0.14.3 (2021-02-05)


#### Bug Fixes

* **client:** HTTP/1 client "Transfer-Encoding" repair code would panic (#2410) ([2c8121f1](https://github.com/hyperium/hyper/commit/2c8121f1735aa8efeb0d5e4ef595363c373ba470), closes [#2409](https://github.com/hyperium/hyper/issues/2409))
* **http1:** fix server misinterpretting multiple Transfer-Encoding headers ([8f93123e](https://github.com/hyperium/hyper/commit/8f93123efef5c1361086688fe4f34c83c89cec02))


#### Features

* **body:**
  * reexport `hyper::body::SizeHint` (#2404) ([9956587f](https://github.com/hyperium/hyper/commit/9956587f83428a5dbe338ba0b55c1dc0bce8c282))
  * add `send_trailers` to Body channel's `Sender` (#2387) ([bf8d74ad](https://github.com/hyperium/hyper/commit/bf8d74ad1cf7d0b33b470b1e61625ebac56f9c4c), closes [#2260](https://github.com/hyperium/hyper/issues/2260))
* **ffi:**
  * add HYPERE_INVALID_PEER_MESSAGE error code for parse errors ([1928682b](https://github.com/hyperium/hyper/commit/1928682b33f98244435ba6d574677546205a15ec))
  * Initial C API for hyper ([3ae1581a](https://github.com/hyperium/hyper/commit/3ae1581a539b67363bd87d9d8fc8635a204eec5d))


### v0.14.2 (2020-12-29)


#### Features

* **client:** expose `connect` types without proto feature (#2377) ([73a59e5f](https://github.com/hyperium/hyper/commit/73a59e5fc7ddedcb7cbd91e97b33385fde57aa10))
* **server:** expose `Accept` without httpX features (#2382) ([a6d4fcbe](https://github.com/hyperium/hyper/commit/a6d4fcbee65bebf461291def75f4c512ec62a664))


### v0.14.1 (2020-12-23)

* Fixes building documentation.


## v0.14.0 (2020-12-23)


#### Bug Fixes

* **client:** log socket option errors instead of returning error (#2361) ([dad5c879](https://github.com/hyperium/hyper/commit/dad5c8792fec7b586b41b5237bc161d8f0c09f72), closes [#2359](https://github.com/hyperium/hyper/issues/2359))
* **http1:**
  * ignore chunked trailers (#2357) ([1dd761c8](https://github.com/hyperium/hyper/commit/1dd761c87de226261599ff2518fe9d231ba1c82d), closes [#2171](https://github.com/hyperium/hyper/issues/2171))
  * ending close-delimited body should close (#2322) ([71f34024](https://github.com/hyperium/hyper/commit/71f340242120f1ea52c7446b4bae37b894b83912))


#### Features

* **client:**
  * change DNS Resolver to resolve to SocketAddrs (#2346) ([b4e24332](https://github.com/hyperium/hyper/commit/b4e24332a0cd44068a806081d51686f50c086056), closes [#1937](https://github.com/hyperium/hyper/issues/1937))
  * Make `client` an optional feature ([4e55583d](https://github.com/hyperium/hyper/commit/4e55583d30a597884883f1a51b678f5c57c76765))
* **http1:** Make HTTP/1 support an optional feature ([2a19ab74](https://github.com/hyperium/hyper/commit/2a19ab74ed69bc776da25544e98979c9fb6e1834))
* **http2:** Make HTTP/2 support an optional feature ([b819b428](https://github.com/hyperium/hyper/commit/b819b428d314f2203642a015545967601b8e518a))
* **lib:**
  * Upgrade to Tokio 1.0, Bytes 1.0, http-body 0.4 (#2369) ([fad42acc](https://github.com/hyperium/hyper/commit/fad42acc79b54ce38adf99c58c894f29fa2665ad), closes [#2370](https://github.com/hyperium/hyper/issues/2370))
  * remove dependency on `tracing`'s `log` feature (#2342) ([db32e105](https://github.com/hyperium/hyper/commit/db32e1050cf1eae63af0365c97e920f1295b6bea), closes [#2326](https://github.com/hyperium/hyper/issues/2326))
  * disable all optional features by default (#2336) ([ed2b22a7](https://github.com/hyperium/hyper/commit/ed2b22a7f66899d338691552fbcb6c0f2f4e06b9))
* **server:** Make the `server` code an optional feature (#2334) ([bdb5e5d6](https://github.com/hyperium/hyper/commit/bdb5e5d6946f4e3f8115a6b1683aff6a04df73de))
* **upgrade:** Moved HTTP upgrades off `Body` to a new API (#2337) ([121c3313](https://github.com/hyperium/hyper/commit/121c33132c0950aaa422848cdc43f6691ddf5785), closes [#2086](https://github.com/hyperium/hyper/issues/2086))


#### Breaking Changes

* hyper depends on `tokio` v1 and `bytes` v1.
* Custom resolvers used with `HttpConnector` must change
  to resolving to an iterator of `SocketAddr`s instead of `IpAddr`s.
 ([b4e24332](https://github.com/hyperium/hyper/commit/b4e24332a0cd44068a806081d51686f50c086056))
* hyper no longer emits `log` records automatically.
  If you need hyper to integrate with a `log` logger (as opposed to `tracing`),
  you can add `tracing = { version = "0.1", features = ["log"] }` to activate them.
 ([db32e105](https://github.com/hyperium/hyper/commit/db32e1050cf1eae63af0365c97e920f1295b6bea))
* Removed `http1_writev` methods from `client::Builder`,
  `client::conn::Builder`, `server::Builder`, and `server::conn::Builder`.
  
  Vectored writes are now enabled based on whether the `AsyncWrite`
  implementation in use supports them, rather than though adaptive
  detection. To explicitly disable vectored writes, users may wrap the IO
  in a newtype that implements `AsyncRead` and `AsyncWrite` and returns
  `false` from its `AsyncWrite::is_write_vectored` method.
 ([d6aadb83](https://github.com/hyperium/hyper/commit/d6aadb830072959497f414c01bcdba4c8e681088))
* The method `Body::on_upgrade()` is gone. It is
  essentially replaced with `hyper::upgrade::on(msg)`.
 ([121c3313](https://github.com/hyperium/hyper/commit/121c33132c0950aaa422848cdc43f6691ddf5785))
* All optional features have been disabled by default.
 ([ed2b22a7](https://github.com/hyperium/hyper/commit/ed2b22a7f66899d338691552fbcb6c0f2f4e06b9))
* The HTTP server code is now an optional feature. To
  enable the server, add `features = ["server"]` to the dependency in
  your `Cargo.toml`.
 ([bdb5e5d6](https://github.com/hyperium/hyper/commit/bdb5e5d6946f4e3f8115a6b1683aff6a04df73de))
* The HTTP client of hyper is now an optional feature. To
  enable the client, add `features = ["client"]` to the dependency in
  your `Cargo.toml`.
 ([4e55583d](https://github.com/hyperium/hyper/commit/4e55583d30a597884883f1a51b678f5c57c76765))
* This puts all HTTP/1 methods and support behind an
  `http1` cargo feature, which will not be enabled by default. To use
  HTTP/1, add `features = ["http1"]` to the hyper dependency in your
  `Cargo.toml`.

 ([2a19ab74](https://github.com/hyperium/hyper/commit/2a19ab74ed69bc776da25544e98979c9fb6e1834))
* This puts all HTTP/2 methods and support behind an
  `http2` cargo feature, which will not be enabled by default. To use
  HTTP/2, add `features = ["http2"]` to the hyper dependency in your
  `Cargo.toml`.

 ([b819b428](https://github.com/hyperium/hyper/commit/b819b428d314f2203642a015545967601b8e518a))


### v0.13.9 (2020-11-02)


#### Bug Fixes

* **client:** fix panic when addrs in ConnectingTcpRemote is empty (#2292) ([01103da5](https://github.com/hyperium/hyper/commit/01103da5d9b15e2a7fdc2f1dfec2c23a890d5c16), closes [#2291](https://github.com/hyperium/hyper/issues/2291))
* **http2:** reschedule keep alive interval timer once a pong is received ([2a938d96](https://github.com/hyperium/hyper/commit/2a938d96aec62603dcb548834676ae2c71ae8be2), closes [#2310](https://github.com/hyperium/hyper/issues/2310))


#### Features

* **client:**
  * add `HttpConnector::set_local_addresses` to set both IPv6 and IPv4 local addrs ( ([fb19f3a8](https://github.com/hyperium/hyper/commit/fb19f3a86997af1c8a31a7d5ce6f2b018c9b5a0d))
  * Add accessors to `Connected` fields (#2290) ([2dc9768d](https://github.com/hyperium/hyper/commit/2dc9768d2d3884afa20c08b7cd8782c870d925d2))


### v0.13.8 (2020-09-18)


#### Bug Fixes

* **http1:** return error if user body ends prematurely ([1ecbcbb1](https://github.com/hyperium/hyper/commit/1ecbcbb119e221f60d37b934b81d18493ebded1b), closes [#2263](https://github.com/hyperium/hyper/issues/2263))


#### Features

* **lib:** Setting `http1_writev(true)` will now force writev queue usage ([187c22af](https://github.com/hyperium/hyper/commit/187c22afb5a13d4fa9a3b938a1d71b11b337ac97), closes [#2282](https://github.com/hyperium/hyper/issues/2282))
* **server:** implement `AsRawFd` for `AddrStream` (#2246) ([b5d5e214](https://github.com/hyperium/hyper/commit/b5d5e21449eb613a3c92dcced6f38d227e405594), closes [#2245](https://github.com/hyperium/hyper/issues/2245))


### v0.13.7 (2020-07-13)


#### Bug Fixes

* **client:** don't panic in DNS resolution when task cancelled (#2229) ([0d0d3635](https://github.com/hyperium/hyper/commit/0d0d3635476ba22e5a2b39b0e4b243f57f1f36d2))


#### Features

* **client:** impl tower_service::Service for &Client (#2089) ([77c3b5bc](https://github.com/hyperium/hyper/commit/77c3b5bc0c0d58ecd9f3c004287f65b8a94cc429))
* **http2:** configure HTTP/2 frame size in the high-level builders too (#2214) ([2354a7ee](https://github.com/hyperium/hyper/commit/2354a7eec352b1f72cd8989d29d73dff211403a1))
* **lib:** Move from `log` to `tracing` in a backwards-compatible way (#2204) ([9832aef9](https://github.com/hyperium/hyper/commit/9832aef9eeaeff8979354d5de04b8706ff79a233))


### v0.13.6 (2020-05-29)


#### Features

* **body:** remove Sync bound for Body::wrap_stream ([042c7706](https://github.com/hyperium/hyper/commit/042c770603a212f22387807efe4fc672959df40c))
* **http2:** allow configuring the HTTP/2 frame size ([b6446456](https://github.com/hyperium/hyper/commit/b64464562a02a642a3cf16ea072f39621da21980))


### v0.13.5 (2020-04-17)


#### Bug Fixes

* **server:** fix panic in Connection::graceful_shutdown ([fce3ddce](https://github.com/hyperium/hyper/commit/fce3ddce4671e7df439a9d8fdc469b079fc07318))


### v0.13.4 (2020-03-20)


#### Bug Fixes

* **http1:** try to drain connection buffer if user drops Body ([d838d54f](https://github.com/hyperium/hyper/commit/d838d54fdf0fc4a613612f68274f3520f333dd8e))


#### Features

* **http2:** add HTTP2 keep-alive support for client and server ([9a8413d9](https://github.com/hyperium/hyper/commit/9a8413d91081ad5a949276f05337e984c455e251))


### v0.13.3 (2020-03-03)


#### Features

* **client:** rename `client::Builder` pool options (#2142) ([a82fd6c9](https://github.com/hyperium/hyper/commit/a82fd6c94aa4ce11fe685f9ccfb85c596d596c6e))
* **http2:** add adaptive window size support using BDP (#2138) ([48102d61](https://github.com/hyperium/hyper/commit/48102d61228b592b466af273a81207e729315681))
* **server:** add `poll_peek` to `AddrStream` (#2127) ([24d53d3f](https://github.com/hyperium/hyper/commit/24d53d3f66f843a6c19204cc7c52cd80e327d41a))


### v0.13.2 (2020-01-29)


#### Bug Fixes

* **body:** return exactly 0 SizeHint for empty body (#2122) ([dc882047](https://github.com/hyperium/hyper/commit/dc88204716664d12e20598c78cb87cd44c6f23af))
* **client:** strip path from Uri before calling Connector (#2109) ([ba2a144f](https://github.com/hyperium/hyper/commit/ba2a144f8b81042247088215425f91760d8694a1))
* **http1:**
  * only send `100 Continue` if request body is polled ([c4bb4db5](https://github.com/hyperium/hyper/commit/c4bb4db5c219459b37d796f9aa2b3cdc93325621))
  * remove panic for HTTP upgrades that have been ignored (#2115) ([1881db63](https://github.com/hyperium/hyper/commit/1881db6391acc949384f8ddfcac8c82a2b133c8d), closes [#2114](https://github.com/hyperium/hyper/issues/2114))
* **http2:** don't add client content-length if method doesn't require it ([fb90d30c](https://github.com/hyperium/hyper/commit/fb90d30c02d8f7cdc9a643597d5c4ca7a123f3dd))


#### Features

* **service:** Implement Clone/Copy on ServiceFn and MakeServiceFn (#2104) ([a5720fab](https://github.com/hyperium/hyper/commit/a5720fab4ced447b8ade43cc1ce8b35442ebf234))


### v0.13.1 (2019-12-13)


#### Bug Fixes

* **http1:** fix response with non-chunked transfer-encoding to be close-delimited ([cb71d2cd](https://github.com/hyperium/hyper/commit/cb71d2cdbd22e538663e724916dc343430efcf29), closes [#2058](https://github.com/hyperium/hyper/issues/2058))


#### Features

* **body:** implement `HttpBody` for `Request` and `Response` ([4b6099c7](https://github.com/hyperium/hyper/commit/4b6099c7aa558e6b1fda146ce6179cb0c67858d7), closes [#2067](https://github.com/hyperium/hyper/issues/2067))
* **client:** expose `hyper::client::connect::Connect` trait alias ([2553ea1a](https://github.com/hyperium/hyper/commit/2553ea1a7ae3d11f0232a5818949146fa3f68a29))


## v0.13.0 (2019-12-10)


#### Bug Fixes

* **client:**
  * fix polling dispatch channel after it has closed ([039281b8](https://github.com/hyperium/hyper/commit/039281b89cf1ab54a0ecc10c5e7fee56d4da0cf4))
  * fix panic from unreachable code ([e6027bc0](https://github.com/hyperium/hyper/commit/e6027bc02db92d1137c54a26eef2e1cb4d810e25))
* **dependencies:** require correct bytes minimum version (#1975) ([536b1e18](https://github.com/hyperium/hyper/commit/536b1e184e9704f50716cf10bf9d4e11a79337da))
* **server:**
  * change `Builder` window size methods to be by-value ([a22dabd0](https://github.com/hyperium/hyper/commit/a22dabd0935e5471fb6b7e511fc9c585ced0a53a), closes [#1814](https://github.com/hyperium/hyper/issues/1814))
  * ignore expect-continue with no body in debug mode ([ca5836f1](https://github.com/hyperium/hyper/commit/ca5836f1ece7c4a67172bcbe72745cb49e8951b0), closes [#1843](https://github.com/hyperium/hyper/issues/1843))
  * Remove unneeded `'static` bound of `Service` on `Connection` (#1971) ([4d147126](https://github.com/hyperium/hyper/commit/4d14712643e4c2ba235a569bb5d9e3099101c1a1))


#### Features

* **body:**
  * change `Sender::send_data` to an `async fn`. ([62a96c07](https://github.com/hyperium/hyper/commit/62a96c077b85792fbf6eb080ec8fec646c47e385))
  * require `Sync` when wrapping a dynamic `Stream` ([44413721](https://github.com/hyperium/hyper/commit/4441372121e8b278ac773ddd4e408a642dadf2d8))
  * add `body::aggregate` and `body::to_bytes` functions ([8ba9a8d2](https://github.com/hyperium/hyper/commit/8ba9a8d2c4bab0f44b3f94a326b3b91c82d7877e))
  * replace `Chunk` type with `Bytes` ([5a598757](https://github.com/hyperium/hyper/commit/5a59875742500672f253719c1e1a16b4eddfacc7), closes [#1931](https://github.com/hyperium/hyper/issues/1931))
  * replace the `Payload` trait with `HttpBody` ([c63728eb](https://github.com/hyperium/hyper/commit/c63728eb38182ad2f93edd729dbf50f3d5c40479))
* **client:**
  * impl tower_service::Service for Client ([edbd10ac](https://github.com/hyperium/hyper/commit/edbd10ac96c5cc6dbeca80ada80f143dbd13d118))
  * provide tower::Service support for clients (#1915) ([eee2a728](https://github.com/hyperium/hyper/commit/eee2a728797346f8c96c15c5958a05432a4e4453))
  * change connectors to return an `impl Connection` ([4d7a2266](https://github.com/hyperium/hyper/commit/4d7a2266b88b2c5c92231bcd2bd75d5842198add))
  * remove `Destination` for `http::Uri` in connectors ([319e8aee](https://github.com/hyperium/hyper/commit/319e8aee1571d8d3639b3259e7a1edb964e6a26c))
  * filter remote IP addresses by family of given local IP address ([131962c8](https://github.com/hyperium/hyper/commit/131962c86ab0a31c2413261cf4532eca88d67dcb))
  * change `Resolve` to be `Service<Name>` ([9d9233ce](https://github.com/hyperium/hyper/commit/9d9233ce7ceddb0fa6f5e725b0a781929add3c58), closes [#1903](https://github.com/hyperium/hyper/issues/1903))
  * change `Connect` trait into an alias for `Service` ([d67e49f1](https://github.com/hyperium/hyper/commit/d67e49f1491327a78f804bab32804dc6c73d2974), closes [#1902](https://github.com/hyperium/hyper/issues/1902))
  * change `GaiResolver` to use a global blocking threadpool ([049b5132](https://github.com/hyperium/hyper/commit/049b5132dbb6199a32e1795d005003f99d0e0b74))
  * Add connect timeout to HttpConnector (#1972) ([4179297a](https://github.com/hyperium/hyper/commit/4179297ac9805af8f84d54525e089ff3f19008ab))
* **lib:**
  * update to `std::future::Future` ([8f4b05ae](https://github.com/hyperium/hyper/commit/8f4b05ae78567dfc52236bc83d7be7b7fc3eebb0))
  * add optional `tcp` feature, split from `runtime` ([5b348b82](https://github.com/hyperium/hyper/commit/5b348b821c3f43d8dd71179862190932fcca6a1c))
  * make `Stream` trait usage optional behind the `stream` feature, enabled by default ([0b03b730](https://github.com/hyperium/hyper/commit/0b03b730531654b1b5f632099386ab27c94eb9f4), closes [#2034](https://github.com/hyperium/hyper/issues/2034))
  * update Tokio, bytes, http, h2, and http-body ([cb3f39c2](https://github.com/hyperium/hyper/commit/cb3f39c2dc6340060f6b17f354f04c872a947574))
* **rt:** introduce `rt::Executor` trait ([6ae5889f](https://github.com/hyperium/hyper/commit/6ae5889f8378b6454d4dc620f33bd1678d0e00e4), closes [#1944](https://github.com/hyperium/hyper/issues/1944))
* **server:**
  * introduce `Accept` trait ([b3e55062](https://github.com/hyperium/hyper/commit/b3e5506261c33dcaca39a126e891a0b9d5df5eea))
  * give `Server::local_addr` a more general type ([3cc93e79](https://github.com/hyperium/hyper/commit/3cc93e796aad59b3996fc26b8839a783e0307925))
  * change `http1_half_close` option default to disabled ([7e31fd88](https://github.com/hyperium/hyper/commit/7e31fd88a86ac032d05670ba4e293e3e5fcccbaf))
* **service:**
    * use tower_service::Service for hyper::service ([ec520d56](https://github.com/hyperium/hyper/commit/ec520d5602d819fd92f497cc230df436c1a39eb0))
  * rename `Service` to `HttpService`, re-export `tower::Service` ([4f274399](https://github.com/hyperium/hyper/commit/4f2743991c227836c3886778512afe1297df3e5b), closes [#1959](https://github.com/hyperium/hyper/issues/1959))


#### Breaking Changes

* All usage of async traits (`Future`, `Stream`,
`AsyncRead`, `AsyncWrite`, etc) are updated to newer versions.

 ([8f4b05ae](https://github.com/hyperium/hyper/commit/8f4b05ae78567dfc52236bc83d7be7b7fc3eebb0))
* All usage of `hyper::Chunk` should be replaced with
  `bytes::Bytes` (or `hyper::body::Bytes`).

 ([5a598757](https://github.com/hyperium/hyper/commit/5a59875742500672f253719c1e1a16b4eddfacc7))
* Using a `Body` as a `Stream`, and constructing one via
  `Body::wrap_stream`, require enabling the `stream` feature.

 ([511ea388](https://github.com/hyperium/hyper/commit/511ea3889b5cceccb3a42aa72465fe38adef71a4))
* Calls to `GaiResolver::new` and `HttpConnector::new` no
  longer should pass an integer argument for the number of threads.

 ([049b5132](https://github.com/hyperium/hyper/commit/049b5132dbb6199a32e1795d005003f99d0e0b74))
* Connectors no longer return a tuple of
  `(T, Connected)`, but a single `T: Connection`.

 ([4d7a2266](https://github.com/hyperium/hyper/commit/4d7a2266b88b2c5c92231bcd2bd75d5842198add))
* All usage of `hyper::client::connect::Destination`
  should be replaced with `http::Uri`.

 ([319e8aee](https://github.com/hyperium/hyper/commit/319e8aee1571d8d3639b3259e7a1edb964e6a26c))
* All usage of `hyper::body::Payload` should be replaced
  with `hyper::body::HttpBody`.

 ([c63728eb](https://github.com/hyperium/hyper/commit/c63728eb38182ad2f93edd729dbf50f3d5c40479))
* Any type passed to the `executor` builder methods must
  now implement `hyper::rt::Executor`.

  `hyper::rt::spawn` usage should be replaced with `tokio::task::spawn`.

  `hyper::rt::run` usage should be replaced with `#[tokio::main]` or
  managing a `tokio::runtime::Runtime` manually.

 ([6ae5889f](https://github.com/hyperium/hyper/commit/6ae5889f8378b6454d4dc620f33bd1678d0e00e4))
* The `Resolve` trait is gone. All custom resolvers should
  implement `tower::Service` instead.

  The error type of `HttpConnector` has been changed away from
  `std::io::Error`.

 ([9d9233ce](https://github.com/hyperium/hyper/commit/9d9233ce7ceddb0fa6f5e725b0a781929add3c58))
* Any manual implementations of `Connect` must instead
  implement `tower::Service<Uri>`.

 ([d67e49f1](https://github.com/hyperium/hyper/commit/d67e49f1491327a78f804bab32804dc6c73d2974))
* The server's behavior will now by default close
  connections when receiving a read EOF. To allow for clients to close
  the read half, call `http1_half_close(true)` when configuring a
  server.

 ([7e31fd88](https://github.com/hyperium/hyper/commit/7e31fd88a86ac032d05670ba4e293e3e5fcccbaf))
* Passing a `Stream` to `Server::builder` or
  `Http::serve_incoming` must be changed to pass an `Accept` instead. The
  `stream` optional feature can be enabled, and then a stream can be
  converted using `hyper::server::accept::from_stream`.

 ([b3e55062](https://github.com/hyperium/hyper/commit/b3e5506261c33dcaca39a126e891a0b9d5df5eea))
* Usage of `send_data` should either be changed to
  async/await or use `try_send_data`.

 ([62a96c07](https://github.com/hyperium/hyper/commit/62a96c077b85792fbf6eb080ec8fec646c47e385))


### v0.12.35 (2019-09-13)


#### Features

* **body:** identify aborted body write errors ([32869224](https://github.com/hyperium/hyper/commit/3286922460ab63d0a804d8170d862ff4ba5951dd))


### v0.12.34 (2019-09-04)


#### Bug Fixes

* **client:** allow client GET requests with explicit body headers ([23fc8b08](https://github.com/hyperium/hyper/commit/23fc8b0806e7fde435ca00479cd5e3c8c5bdeee7), closes [#1925](https://github.com/hyperium/hyper/issues/1925))


### v0.12.33 (2019-09-04)


### v0.12.32 (2019-07-08)


#### Features

* **client:** `HttpConnector`: allow to set socket buffer sizes ([386109c4](https://github.com/hyperium/hyper/commit/386109c421c21e6e2d70e76d7dd072ef3bb62c58))


### v0.12.31 (2019-06-25)


### v0.12.30 (2019-06-14)


#### Bug Fixes

* **http1:** force always-ready connections to yield after a few spins ([8316f96d](https://github.com/hyperium/hyper/commit/8316f96d807454b76cde3cc6a7be552c02000529))
* **http2:** correctly propagate HTTP2 request cancellation ([50198851](https://github.com/hyperium/hyper/commit/50198851a2b1e47c5ad60565eacb712fb3df1ad6))


### v0.12.29 (2019-05-16)


#### Bug Fixes

* **server:** skip automatic Content-Length header for HTTP 304 responses ([b342c38f](https://github.com/hyperium/hyper/commit/b342c38f08972fe8be4ef9844e30f1e7a121bbc4), closes [#1797](https://github.com/hyperium/hyper/issues/1797))


#### Features

* **body:** implement `http_body::Body` for `hyper::Body` ([2d9f3490](https://github.com/hyperium/hyper/commit/2d9f3490aa04393a12854680aa3e6d6117ba2407))
* **client:** Implement `TryFrom` for `Destination` (#1810) ([d1183a80](https://github.com/hyperium/hyper/commit/d1183a80278decf3955874629e9cff427edecb05), closes [#1808](https://github.com/hyperium/hyper/issues/1808))
* **server:** add initial window builder methods that take self by-val (#1817) ([8b45af7f](https://github.com/hyperium/hyper/commit/8b45af7f314cea7d1db5cb6990088dd8442aa87b))


### v0.12.28 (2019-04-29)


#### Bug Fixes

* **client:**
  * detect HTTP2 connection closures sooner ([e0ec5cad](https://github.com/hyperium/hyper/commit/e0ec5cad9ae3eaa5d9fffeeb636b1363029fcb9c))
  * fix a rare connection pool race condition ([4133181b](https://github.com/hyperium/hyper/commit/4133181bb20f8d7e990994b2119c590f832a95f1))


#### Features

* **server:** impl Sink for Body::Sender ([8d70baca](https://github.com/hyperium/hyper/commit/8d70baca611869c1997571e8513717396b13328b), closes [#1781](https://github.com/hyperium/hyper/issues/1781))


### v0.12.27 (2019-04-10)


#### Bug Fixes

* **http2:** fix import of h2::Reason to work on 1.26 ([5680d944](https://github.com/hyperium/hyper/commit/5680d9441903d6c8d17c19b3ea1e054af76bb08d))


### v0.12.26 (2019-04-09)


#### Bug Fixes

* **http2:** send a GOAWAY when the user's Service::poll_ready errors ([42c5efc0](https://github.com/hyperium/hyper/commit/42c5efc085ac71223e4b57d0e1b866e64d41f4e5))
* **server:** prohibit the length headers on successful CONNECT ([d1501a0f](https://github.com/hyperium/hyper/commit/d1501a0fd3b616d3e42459fc83bdd7ebd01d217e), closes [#1783](https://github.com/hyperium/hyper/issues/1783))


#### Features

* **http2:** check `Error::source()` for an HTTP2 error code to send in reset ([fc18b680](https://github.com/hyperium/hyper/commit/fc18b680a5656a0c31bc09c1c70571956a1fd013))


### v0.12.25 (2019-03-01)


#### Bug Fixes

* **client:** coerce HTTP_2 requests to HTTP_11 ([3a6080b1](https://github.com/hyperium/hyper/commit/3a6080b14abecc29c9aed77be6d60d34a12b368c), closes [#1770](https://github.com/hyperium/hyper/issues/1770))
* **http2:** send INTERNAL_ERROR when user's Service errors ([8f926a0d](https://github.com/hyperium/hyper/commit/8f926a0daeaf4716cfb2e6db143c524da34421de))


#### Features

* **error:** implement `Error::source` when available ([4cf22dfa](https://github.com/hyperium/hyper/commit/4cf22dfa2139f072e0ee937de343a0b0b0a77a22), closes [#1768](https://github.com/hyperium/hyper/issues/1768))
* **http2:** Add window size config options for Client and Server ([7dcd4618](https://github.com/hyperium/hyper/commit/7dcd4618c059cc76987a32d3acb75e2aaed4419e), closes [#1771](https://github.com/hyperium/hyper/issues/1771))
* **server:** add `http2_max_concurrent_streams` builder option ([cbae4294](https://github.com/hyperium/hyper/commit/cbae4294c416a64b56f30be7b6494f9934016d1e), closes [#1772](https://github.com/hyperium/hyper/issues/1772))
* **service:**
  * add `poll_ready` to `Service` and `MakeService` (#1767) ([0bf30ccc](https://github.com/hyperium/hyper/commit/0bf30ccc68feefb0196d2db9536232e5913598da))
  * allow `FnMut` with `service_fn` ([877606d5](https://github.com/hyperium/hyper/commit/877606d5c81195374259561aa98b973a00fa6056))


### v0.12.24 (2019-02-11)


#### Bug Fixes

* **client:** fix panic when CONNECT request doesn't have a port ([d16b2c30](https://github.com/hyperium/hyper/commit/d16b2c30810a2d96ab226997930d953b2fc2626b))


#### Features

* **server:**
  * add `http1_max_buf_size` in the `server::Builder` (#1761) ([3e9782c2](https://github.com/hyperium/hyper/commit/3e9782c2a9501a3122df8a54775a1fa7f2386fea))
  * add `into_inner` to `AddrStream` (#1762) ([e52f80df](https://github.com/hyperium/hyper/commit/e52f80df5a114844d239561218112a650067f006))


### v0.12.23 (2019-01-24)


#### Bug Fixes

* **http2:** revert http2 refactor causing a client hang ([9aa7e990](https://github.com/hyperium/hyper/commit/9aa7e99010a1a0f086ade27f99cf4b8da00ae750))


#### Features

* **client:** add `conn::Builder::max_buf_size()` ([078ed82d](https://github.com/hyperium/hyper/commit/078ed82dd5fed2f6c4399ff041ef116c712eaaf8), closes [#1748](https://github.com/hyperium/hyper/issues/1748))


### v0.12.22 (2019-01-23)


#### Bug Fixes

* **client:** parse IPv6 hosts correctly in HttpConnector ([c328c62e](https://github.com/hyperium/hyper/commit/c328c62ec29cd328c1c7331bb316fe4a548f11d7))


### v0.12.21 (2019-01-15)


#### Features

* **client:**
  * add `Destination::try_from_uri` constructor ([c809542c](https://github.com/hyperium/hyper/commit/c809542c830c8d542877a22dd54b1c5c679ae433))
  * Add useful trait impls to Name ([be5ec455](https://github.com/hyperium/hyper/commit/be5ec45571e0b1c6c2b20fe4ab49ef1b0226a004))
  * add FromStr impl for Name ([607c4da0](https://github.com/hyperium/hyper/commit/607c4da0b96ca430593599c928c882a17a7914d5))


### v0.12.20 (2019-01-07)


#### Bug Fixes

* **dependencies:** disable unneeded optional tokio features ([e5135dd6](https://github.com/hyperium/hyper/commit/e5135dd6f619b5817e31572c98b45d7c4b34f43a), closes [#1739](https://github.com/hyperium/hyper/issues/1739))
* **http2:** don't consider an h2 send request error as canceled ([cf034e99](https://github.com/hyperium/hyper/commit/cf034e99fa895fdf4b66edf392f8c7ca366448fd))


### v0.12.19 (2018-12-18)


#### Bug Fixes

* **rt:** prevent fallback reactor thread from being created accidentally ([1d253b4d](https://github.com/hyperium/hyper/commit/1d253b4d4759e045409fcf140adda7d327a05c8a))


### v0.12.18 (2018-12-11)


#### Features

* **server:** add `server::conn::AddrIncoming::bind` constructor ([2d5eabde](https://github.com/hyperium/hyper/commit/2d5eabdeed06ea1c88d88dff464929616710ee9a))


### v0.12.17 (2018-12-05)


#### Features

* **error:** add `Error::is_connect` method ([01f64983](https://github.com/hyperium/hyper/commit/01f64983559602b9ebaaeecf6d33e97a88185676))
* **server:**
  * add `tcp_sleep_on_accept_errors` builder method ([a6fff13a](https://github.com/hyperium/hyper/commit/a6fff13a392d3394cacb1215f83bd8ec87671566), closes [#1713](https://github.com/hyperium/hyper/issues/1713))
  * add `http1_half_close(bool)` option ([73345be6](https://github.com/hyperium/hyper/commit/73345be65f895660492e28e718786b66034a4d03), closes [#1716](https://github.com/hyperium/hyper/issues/1716))
* **service:** export `hyper::service::MakeServiceRef` ([a522c315](https://github.com/hyperium/hyper/commit/a522c3151abd11795d3263f6607a7caf7c19a585))

#### Performance

* **http1:** implement an adaptive read buffer strategy which helps with throughput and memory management ([fd25129d](https://github.com/hyperium/hyper/commit/fd25129dc0e543538ccbd1794d22014bc187e050), closes [#1708](https://github.com/hyperium/hyper/issues/1708))

### v0.12.16 (2018-11-21)


#### Bug Fixes

* **client:** fix connection leak when Response finishes before Request body ([e455fa24](https://github.com/hyperium/hyper/commit/e455fa2452cf45d66de6b4c3dc567e2b5d2368a4), closes [#1717](https://github.com/hyperium/hyper/issues/1717))


#### Features

* **client:** add `http1_read_buf_exact_size` Builder option ([2e7250b6](https://github.com/hyperium/hyper/commit/2e7250b6698407b97961b8fcae78696e94d6ea57))


### v0.12.15 (2018-11-20)


#### Features

* **client:** add client::conn::Builder::executor method ([95446cc3](https://github.com/hyperium/hyper/commit/95446cc338f8055539dd3503c482d649f42a531c))
* **server:** change `NewService` to `MakeService` with connection context ([30870029](https://github.com/hyperium/hyper/commit/30870029b9eb162f566d8dddd007fb6df9cd69af), closes [#1650](https://github.com/hyperium/hyper/issues/1650))


### v0.12.14 (2018-11-07)


#### Bug Fixes

* **header:** fix panic when parsing header names larger than 64kb ([9245e940](https://github.com/hyperium/hyper/commit/9245e9409aeb5bb3e31b7f7c0e125583d1318465))


#### Features

* **client:** add ALPN h2 support for client connectors ([976a77a6](https://github.com/hyperium/hyper/commit/976a77a67360a2590699c0b2bb3a4c3ccc0ff1ba))


### v0.12.13 (2018-10-26)


#### Features

* **client:**
  * add `Resolve`, used by `HttpConnector` ([2d5af177](https://github.com/hyperium/hyper/commit/2d5af177c1f0cfa3f592eec56f3a971fd9770f72), closes [#1517](https://github.com/hyperium/hyper/issues/1517))
  * adds `HttpInfo` to responses when `HttpConnector` is used ([13d53e1d](https://github.com/hyperium/hyper/commit/13d53e1d0c095a61f64ff1712042aa615122d33d), closes [#1402](https://github.com/hyperium/hyper/issues/1402))
* **dns:**
  * export `client::connect::dns` module, and `TokioThreadpoolGaiResolver` type. ([34d780ac](https://github.com/hyperium/hyper/commit/34d780acd0fd7fe6a41b3eca1641791c7a33b366))
  * tokio_threadpool::blocking resolver ([1e8d6439](https://github.com/hyperium/hyper/commit/1e8d6439cf4f9c7224fe80f0aeee32e2af1adbb0), closes [#1676](https://github.com/hyperium/hyper/issues/1676))
* **http:** reexport `http` crate ([d55b5efb](https://github.com/hyperium/hyper/commit/d55b5efb890ef04e37825221deae9c57e9e602fa))
* **server:** allow `!Send` Servers ([ced949cb](https://github.com/hyperium/hyper/commit/ced949cb6b798f25c2ffbdb3ebda6858c18393a7))


### v0.12.12 (2018-10-16)


#### Bug Fixes

* **armv7:** split record_header_indices loop to work around rustc/LLVM bug ([30a4f237](https://github.com/hyperium/hyper/commit/30a4f2376a392e50ade48685f92e930385ebb68f))
* **http2:** add Date header if not present for HTTP2 server responses ([37ec724f](https://github.com/hyperium/hyper/commit/37ec724fd6405dd97c5873dddc956df1711b29ab))
* **server:** log and ignore connection errors on newly accepted sockets ([66a857d8](https://github.com/hyperium/hyper/commit/66a857d801c1fc82d35b6da2d27441aa046aae47))


### v0.12.11 (2018-09-28)


#### Bug Fixes

* **client:** allow calling `Destination::set_host` with IPv6 addresses ([af5e4f3e](https://github.com/hyperium/hyper/commit/af5e4f3ec24a490e209e3e73f86207b63ce7191a), closes [#1661](https://github.com/hyperium/hyper/issues/1661))
* **server:** use provided executor if fallback to HTTP2 ([1370a6f8](https://github.com/hyperium/hyper/commit/1370a6f8f06f9906ff75dec904ab9c6d763e37f0))


### v0.12.10 (2018-09-14)


#### Bug Fixes

* **http1:** fix title-case option when header names have symbols ([ca5e520e](https://github.com/hyperium/hyper/commit/ca5e520e7aa6d0a211e3c152c09095d35326ca12))


### v0.12.9 (2018-08-28)


#### Bug Fixes

* **http2:** allow TE "trailers" request headers ([24f11a42](https://github.com/hyperium/hyper/commit/24f11a421d8422714bf023a602d7718b885a39a0), closes [#1642](https://github.com/hyperium/hyper/issues/1642))
* **server:** properly handle keep-alive for HTTP/1.0 ([1448e406](https://github.com/hyperium/hyper/commit/1448e4067b10da6fe4584921314afc1f5f4e3c8d), closes [#1614](https://github.com/hyperium/hyper/issues/1614))


#### Features

* **client:** add `max_idle_per_host` configuration option ([a3c44ded](https://github.com/hyperium/hyper/commit/a3c44ded556b7ef9487ec48cf42fa948d64f5a83))
* **server:** add `Server::with_graceful_shutdown` method ([168c7d21](https://github.com/hyperium/hyper/commit/168c7d2155952ba09f781c331fd67593b820af20), closes [#1575](https://github.com/hyperium/hyper/issues/1575))


### v0.12.8 (2018-08-10)


#### Bug Fixes

* **server:** coerce responses with HTTP2 version to HTTP/1.1 when protocol is 1.x ([195fbb2a](https://github.com/hyperium/hyper/commit/195fbb2a3728460e7f7eca2035461ce055db6cd0))


#### Features

* **server:**
  * add Builder::http1_keepalive method ([b459adb4](https://github.com/hyperium/hyper/commit/b459adb43a753ba082f1fc03c90ff4e76625666f))
  * add `Server::from_tcp` constructor ([bb4c5e24](https://github.com/hyperium/hyper/commit/bb4c5e24c846995b66e361d1c2446cb81984bbbd), closes [#1602](https://github.com/hyperium/hyper/issues/1602))
  * add remote_addr method to AddrStream ([26f3a5ed](https://github.com/hyperium/hyper/commit/26f3a5ed317330db39dd33f49bafd859bc867d8a))


### v0.12.7 (2018-07-23)


#### Bug Fixes

* **http1:** reduce closed connections when body is dropped ([6530a00a](https://github.com/hyperium/hyper/commit/6530a00a8e3449a8fd7e4ed6ad1231b6b1579c38))


### v0.12.6 (2018-07-11)


#### Features

* **client:**
  * add ability to include `SO_REUSEADDR` option on sockets ([13862d11](https://github.com/hyperium/hyper/commit/13862d11ad329e5198622ad3e924e1aa05ab2c8a), closes [#1599](https://github.com/hyperium/hyper/issues/1599))
  * implement rfc 6555 (happy eyeballs) ([02a9c29e](https://github.com/hyperium/hyper/commit/02a9c29e2e816c8a583f65b372fcf7b8503e6bad))
* **server:** add `Builder::http1_pipeline_flush` configuration ([5b5e3090](https://github.com/hyperium/hyper/commit/5b5e3090955c1b6c1e7a8cb97b43de8d099f5303))


### v0.12.5 (2018-06-28)


### v0.12.4 (2018-06-28)


#### Bug Fixes

* **client:**
  * fix keep-alive header detection when parsing responses ([c03c39e0](https://github.com/hyperium/hyper/commit/c03c39e0ffca94bce265db92281a50b2abae6f2b))
  * try to reuse connections when pool checkout wins ([f2d464ac](https://github.com/hyperium/hyper/commit/f2d464ac79b47f988bffc826b80cf7d107f80694))


### v0.12.3 (2018-06-25)


#### Features

* **client:** enable CONNECT requests through the `Client` ([2a3844ac](https://github.com/hyperium/hyper/commit/2a3844acc393d42ff1b75f798dcc321a20956bea))
* **http2:** quickly cancel when receiving RST_STREAM ([ffdb4788](https://github.com/hyperium/hyper/commit/ffdb47883190a8889cf30b716294383392a763c5))


### v0.12.2 (2018-06-19)


#### Bug Fixes

* **http2:**
  * implement `graceful_shutdown` for HTTP2 server connections ([b7a0c2d5](https://github.com/hyperium/hyper/commit/b7a0c2d5967d9ca22bd5e031166876c81ae80606), closes [#1550](https://github.com/hyperium/hyper/issues/1550))
  * send trailers if Payload includes them ([3affe2a0](https://github.com/hyperium/hyper/commit/3affe2a0af445a01acb75181b16e71eb9fef4ae2))
* **lib:** return an error instead of panic if execute fails ([482a5f58](https://github.com/hyperium/hyper/commit/482a5f589ea2bdb798f01645653975089f40ef44), closes [#1566](https://github.com/hyperium/hyper/issues/1566))
* **server:**
  * fix debug assert failure when kept-alive connections see a parse error ([396fe80e](https://github.com/hyperium/hyper/commit/396fe80e76840dea9373ca448b20cf7a9babd2f8))
  * correctly handle CONNECT requests ([d7ab0166](https://github.com/hyperium/hyper/commit/d7ab01667659290784bfe685951c83a6f69e415e))


#### Features

* **body:**
  * make `Body` know about incoming `Content-Length` ([a0a0fcdd](https://github.com/hyperium/hyper/commit/a0a0fcdd9b126ee2c0810b2839c7ab847f5788ad), closes [#1545](https://github.com/hyperium/hyper/issues/1545))
  * add `Sender::abort` ([a096799c](https://github.com/hyperium/hyper/commit/a096799c1b4581ce1a47ed0817069997a9031828))
* **client:** add `set_scheme`, `set_host`, and `set_port` for `Destination` ([27db8b00](https://github.com/hyperium/hyper/commit/27db8b0061f85d89ec94e07295463e8d1030d94f), closes [#1564](https://github.com/hyperium/hyper/issues/1564))
* **error:** add `Error::cause2` and `Error::into_cause` ([bc5e22f5](https://github.com/hyperium/hyper/commit/bc5e22f58095f294333f49f12eeb7e504cda666c), closes [#1542](https://github.com/hyperium/hyper/issues/1542))
* **http1:** Add higher-level HTTP upgrade support to Client and Server (#1563) ([fea29b29](https://github.com/hyperium/hyper/commit/fea29b29e2bbbba10760917a234a8cf4a6133be4), closes [#1395](https://github.com/hyperium/hyper/issues/1395))
* **http2:**
  * implement flow control for h2 bodies ([1c3fbfd6](https://github.com/hyperium/hyper/commit/1c3fbfd6bf6b627f75ef694e69c8074745276e9b), closes [#1548](https://github.com/hyperium/hyper/issues/1548))
  * Add `content_length()` value to incoming h2 `Body` ([9a28268b](https://github.com/hyperium/hyper/commit/9a28268b98f30fd25e862b4a182a853a9a6e1841), closes [#1546](https://github.com/hyperium/hyper/issues/1546))
  * set Content-Length header on outgoing messages ([386fc0d7](https://github.com/hyperium/hyper/commit/386fc0d70b70d36ac44ec5562cd26babdfd46fc9), closes [#1547](https://github.com/hyperium/hyper/issues/1547))
  * Strip connection headers before sending ([f20afba5](https://github.com/hyperium/hyper/commit/f20afba57d6fabb04085968342e5fd62b45bc8df))


### v0.12.1 (2018-06-04)


#### Bug Fixes

* **server:** add upgrading process to `poll_without_shutdown()` (#1530) ([c6e90b7b](https://github.com/hyperium/hyper/commit/c6e90b7b6509276c744b531f8b1f7b043059c4ec))


#### Features

* **client:** implement `Clone` for `Destination` ([15188b7c](https://github.com/hyperium/hyper/commit/15188b7c7fc6774301a16923127df596486cc913))
* **server:**
  * add `http1_writev` config option for servers ([810435f1](https://github.com/hyperium/hyper/commit/810435f1469eb028c6a819368d63edb54d6c341c), closes [#1527](https://github.com/hyperium/hyper/issues/1527))
  * add `http1_only` configuration ([14d9246d](https://github.com/hyperium/hyper/commit/14d9246de2e97908c915caf254a37fd62edb25d3), closes [#1512](https://github.com/hyperium/hyper/issues/1512))
  * add `try_into_parts()` to `conn::Connection` (#1531) ([c615a324](https://github.com/hyperium/hyper/commit/c615a3242f2518bc8acf05116ebe87ea98773c28))


## v0.12.0 (2018-06-01)

#### Features

* **lib:**
  * add HTTP/2 support for Client and Server ([c119097f](https://github.com/hyperium/hyper/commit/c119097fd072db51751b100fa186b6f64785954d))
  * convert to use tokio 0.1 ([27b8db3a](https://github.com/hyperium/hyper/commit/27b8db3af8852ba8280a2868f703d3230a1db85e))
  * replace types with those from `http` crate ([3cd48b45](https://github.com/hyperium/hyper/commit/3cd48b45fb622fb9e69ba773e7f92b9d3e9ac018))
* **body:**
  * remove `Body::is_empty()` ([19f90242](https://github.com/hyperium/hyper/commit/19f90242f8a3768b2d8d4bff4044a2d6c77d40aa))
  * change `Payload::Data` to be a `Buf` ([a3be110a](https://github.com/hyperium/hyper/commit/a3be110a55571a1ee9a31b2335d7aec27c04e96a), closes [#1508](https://github.com/hyperium/hyper/issues/1508))
  * add `From<Box<Stream>>` impl for `Body` ([45efba27](https://github.com/hyperium/hyper/commit/45efba27df90650bf4669738102ad6e432ddc75d))
  * introduce a `Payload` trait to represent bodies ([fbc449e4](https://github.com/hyperium/hyper/commit/fbc449e49cc4a4f8319647dccfb288d3d83df2bd), closes [#1438](https://github.com/hyperium/hyper/issues/1438))
* **client:**
  * rename `FutureResponse` to `ResponseFuture` ([04c74ef5](https://github.com/hyperium/hyper/commit/04c74ef596eb313b785ecad6c42c0375ddbb1e96))
  * support local bind for `HttpConnector` ([b6a3c85d](https://github.com/hyperium/hyper/commit/b6a3c85d0f9ede10759dc2309502e88ea3e513f7), closes [#1498](https://github.com/hyperium/hyper/issues/1498))
  * add support for title case header names (#1497) ([a02fec8c](https://github.com/hyperium/hyper/commit/a02fec8c7898792cbeadde7e0f5bf111d55dd335), closes [#1492](https://github.com/hyperium/hyper/issues/1492))
  * add support to set `SO_NODELAY` on client HTTP sockets ([016d79ed](https://github.com/hyperium/hyper/commit/016d79ed2633e3f939a2cd10454cbfc5882effb4), closes [#1473](https://github.com/hyperium/hyper/issues/1473))
  * improve construction of `Client`s ([fe1578ac](https://github.com/hyperium/hyper/commit/fe1578acf628844d7cccb3e896c5e0bb2a0be729))
  * redesign the `Connect` trait ([8c52c2df](https://github.com/hyperium/hyper/commit/8c52c2dfd342e798420a0b83cde7d54f3af5e351), closes [#1428](https://github.com/hyperium/hyper/issues/1428))
* **error:** revamp `hyper::Error` type ([5d3c4722](https://github.com/hyperium/hyper/commit/5d3c472228d40b57e47ea26004b3710cfdd451f3), closes [#1128](https://github.com/hyperium/hyper/issues/1128), [#1130](https://github.com/hyperium/hyper/issues/1130), [#1431](https://github.com/hyperium/hyper/issues/1431), [#1338](https://github.com/hyperium/hyper/issues/1338))
* **rt:** make tokio runtime optional ([d127201e](https://github.com/hyperium/hyper/commit/d127201ef22b10ab1d84b3f2215863eb2d03bfcb))
* **server:**
  * support HTTP1 and HTTP2 automatically ([bc6af88a](https://github.com/hyperium/hyper/commit/bc6af88a32e29e5a4f3719d8abc664f9ab10dddd), closes [#1486](https://github.com/hyperium/hyper/issues/1486))
  * re-design `Server` as higher-level API ([c4974500](https://github.com/hyperium/hyper/commit/c4974500abee45b95b0b54109cad15978ef8ced9), closes [#1322](https://github.com/hyperium/hyper/issues/1322), [#1263](https://github.com/hyperium/hyper/issues/1263))
* **service:** introduce hyper-specific `Service` ([2dc6202f](https://github.com/hyperium/hyper/commit/2dc6202fe7294fa74cf1ba58a45e48b8a927934f), closes [#1461](https://github.com/hyperium/hyper/issues/1461))

#### Bug Fixes

* **lib:** remove deprecated tokio-proto APIs ([a37e6b59](https://github.com/hyperium/hyper/commit/a37e6b59e6d6936ee31c6d52939869933c709c78))
* **server:** panic on max_buf_size too small ([aac250f2](https://github.com/hyperium/hyper/commit/aac250f29d3b05d8c07681a407825811ec6a0b56))

#### Breaking Changes

* `Body::is_empty()` is gone. Replace with
  `Body::is_end_stream()`, from the `Payload` trait.

  ([19f90242](https://github.com/hyperium/hyper/commit/19f90242f8a3768b2d8d4bff4044a2d6c77d40aa))
* Each payload chunk must implement `Buf`, instead of
  just `AsRef<[u8]>`.

  ([a3be110a](https://github.com/hyperium/hyper/commit/a3be110a55571a1ee9a31b2335d7aec27c04e96a))
* Replace any references of
  `hyper::client::FutureResponse` to `hyper::client::ResponseFuture`.

  ([04c74ef5](https://github.com/hyperium/hyper/commit/04c74ef596eb313b785ecad6c42c0375ddbb1e96))
* The `Service` trait has changed: it has some changed
  associated types, and `call` is now bound to `&mut self`.

  The `NewService` trait has changed: it has some changed associated
  types, and `new_service` now returns a `Future`.

  `Client` no longer implements `Service` for now.

  `hyper::server::conn::Serve` now returns `Connecting` instead of
  `Connection`s, since `new_service` can now return a `Future`. The
  `Connecting` is a future wrapping the new service future, returning
  a `Connection` afterwards. In many cases, `Future::flatten` can be
  used.

  ([2dc6202f](https://github.com/hyperium/hyper/commit/2dc6202fe7294fa74cf1ba58a45e48b8a927934f))
* The `Server` is no longer created from `Http::bind`,
  nor is it `run`. It is a `Future` that must be polled by an
  `Executor`.

  The `hyper::server::Http` type has move to
  `hyper::server::conn::Http`.

  ([c4974500](https://github.com/hyperium/hyper/commit/c4974500abee45b95b0b54109cad15978ef8ced9))
* `Client:new(&handle)` and `Client::configure()` are now
  `Client::new()` and `Client::builder()`.

  ([fe1578ac](https://github.com/hyperium/hyper/commit/fe1578acf628844d7cccb3e896c5e0bb2a0be729))
* `Error` is no longer an enum to pattern match over, or
  to construct. Code will need to be updated accordingly.

  For body streams or `Service`s, inference might be unable to determine
  what error type you mean to return.

  ([5d3c4722](https://github.com/hyperium/hyper/commit/5d3c472228d40b57e47ea26004b3710cfdd451f3))
* All uses of `Handle` now need to be new-tokio `Handle`.

  ([27b8db3a](https://github.com/hyperium/hyper/commit/27b8db3af8852ba8280a2868f703d3230a1db85e))
* Custom connectors should now implement `Connect`
  directly, instead of `Service`.

  Calls to `connect` no longer take `Uri`s, but `Destination`. There
  are `scheme`, `host`, and `port` methods to query relevant
  information.

  The returned future must be a tuple of the transport and `Connected`.
  If no relevant extra information is needed, simply return
  `Connected::new()`.

  ([8c52c2df](https://github.com/hyperium/hyper/commit/8c52c2dfd342e798420a0b83cde7d54f3af5e351))
* All code that was generic over the body as `Stream` must
  be adjusted to use a `Payload` instead.

  `hyper::Body` can still be used as a `Stream`.

  Passing a custom `impl Stream` will need to either implement
  `Payload`, or as an easier option, switch to `Body::wrap_stream`.

  `Body::pair` has been replaced with `Body::channel`, which returns a
  `hyper::body::Sender` instead of a `futures::sync::mpsc::Sender`.

  ([fbc449e4](https://github.com/hyperium/hyper/commit/fbc449e49cc4a4f8319647dccfb288d3d83df2bd))
* `Method`, `Request`, `Response`, `StatusCode`,
  `Version`, and `Uri` have been replaced with types from the `http`
  crate.

  ([3cd48b45](https://github.com/hyperium/hyper/commit/3cd48b45fb622fb9e69ba773e7f92b9d3e9ac018))
  * The variants of `Method` are now uppercase, for instance, `Method::Get` is now `Method::GET`.
  * The variants of `StatusCode` are now uppercase, for instance, `StatusCode::Ok` is now `StatusCode::OK`.
  * The variants of `Version` are now uppercase, for instance, `HttpVersion::Http11` is now `Version::HTTP_11`.
*  The typed headers from `hyper::header` are gone for now.

  The `http::header` module is re-exported as `hyper::header`.

  For example, a before setting the content-length:

  ```rust
  use hyper::header::ContentLength;
  res.headers_mut().set(ContentLength(15));
  ```

  And now **after**, with the `http` types:

  ```rust
  use hyper::header::{CONTENT_LENGTH, HeaderValue};
  res.headers_mut().insert(CONTENT_LENGTH, HeaderValue::from_static("15"));
  ```

  ([3cd48b45](https://github.com/hyperium/hyper/commit/3cd48b45fb622fb9e69ba773e7f92b9d3e9ac018))
* The `mime` crate is no longer re-exported as `hyper::mime`.

  The typed headers don't exist, and so they do not need the `mime` crate.

  To continue using `mime` for other purposes, add it directly to your `Cargo.toml`
  as a dependency.

  ([3cd48b45](https://github.com/hyperium/hyper/commit/3cd48b45fb622fb9e69ba773e7f92b9d3e9ac018))
* Removed `compat` cargo feature, and `compat` related API. This was the conversion methods for hyper's
  types to and from `http` crate's types.

  ([3cd48b45](https://github.com/hyperium/hyper/commit/3cd48b45fb622fb9e69ba773e7f92b9d3e9ac018))
* Removed deprecated APIs:
  ([a37e6b59](https://github.com/hyperium/hyper/commit/a37e6b59e6d6936ee31c6d52939869933c709c78))
  * The `server-proto` cargo feature, which included `impl ServerProto for Http`, and related associated types.
  * `client::Config::no_proto()`
  * `tokio_proto::streaming::Body::from(hyper::Body)`
  * `hyper::Body::from(tokio_proto::streaming::Body)`
  * `hyper::Body::from(futures::sync::mpsc::Receiver)`
  * `Http::no_proto()`


### v0.11.27 (2018-05-16)


#### Bug Fixes

* **client:** prevent pool checkout looping on not-ready connections ([ccec79da](https://github.com/hyperium/hyper/commit/ccec79dadc84f1e9fced9159189d9f8caa6e17a4), closes [#1519](https://github.com/hyperium/hyper/issues/1519))
* **server:** skip SO_REUSEPORT errors ([2c48101a](https://github.com/hyperium/hyper/commit/2c48101a6ee1269d7c94a0c3e606b2d635b20615), closes [#1509](https://github.com/hyperium/hyper/issues/1509))


### v0.11.26 (2018-05-05)


#### Features

* **server:** add Server::run_threads to run on multiple threads ([8b644c1a](https://github.com/hyperium/hyper/commit/8b644c1a2a1a629be9b263d8fae5963a61af91cd))


### v0.11.25 (2018-04-04)


#### Bug Fixes

* **client:** ensure idle connection is pooled before response body finishes ([7fe9710a](https://github.com/hyperium/hyper/commit/7fe9710a98650efc37f35bb21b19926c015f0631))


### v0.11.24 (2018-03-22)


#### Bug Fixes

* **header:** remove charset=utf8 from `ContentType::text()` ([ba789e65](https://github.com/hyperium/hyper/commit/ba789e6552eb74afb98f4d462d5c06c6643535d3))


### v0.11.23 (2018-03-22)


#### Bug Fixes

* **server:** prevent to output Transfer-encoding when server upgrade (#1465) ([eb105679](https://github.com/hyperium/hyper/commit/eb105679271a6e0ccc09f37978314a1a8d686217))


#### Features

* **client:** introduce lower-level Connection API ([1207c2b6](https://github.com/hyperium/hyper/commit/1207c2b62456fc729c3a29c56c3966b319b474a9), closes [#1449](https://github.com/hyperium/hyper/issues/1449))
* **header:** add `text()` and `text_utf8()` constructors to `ContentType` ([45cf8c57](https://github.com/hyperium/hyper/commit/45cf8c57c932a2756365748dc1e598ad3ee4b8ef))
* **server:**
  * add `service` property to `server::conn::Parts` ([bf7c0bbf](https://github.com/hyperium/hyper/commit/bf7c0bbf4f55fdf465407874b0b2d4bd748e6783), closes [#1471](https://github.com/hyperium/hyper/issues/1471))
    * add upgrade support to lower-level Connection API (#1459) ([d58aa732](https://github.com/hyperium/hyper/commit/d58aa73246112f69410cc3fe912622f284427067), closes [#1323](https://github.com/hyperium/hyper/issues/1323))


### v0.11.22 (2018-03-07)


#### Bug Fixes

* **client:** return error if Request has `CONNECT` method ([bfcdbd9f](https://github.com/hyperium/hyper/commit/bfcdbd9f86480cf6531544ecca247562a18172af))
* **dependencies:** require tokio-core 0.1.11 ([49fcb066](https://github.com/hyperium/hyper/commit/49fcb0663cc30bbfc82cfc3c8e42d539211a3f3f))


#### Features

* **client:** add `Config::set_host` option ([33a385c6](https://github.com/hyperium/hyper/commit/33a385c6b677cce4ece2843c11ac78711fd5b898))


### v0.11.21 (2018-02-28)


#### Bug Fixes

* **client:**
  * check conn is closed in expire interval ([2fa0c845](https://github.com/hyperium/hyper/commit/2fa0c845b5f3f07e039522a9112a14593e02fe1b))
  * schedule interval to clear expired idle connections ([727b7479](https://github.com/hyperium/hyper/commit/727b74797e5754af8abba8812a876c3c8fda6d94))
  * never call connect if idle connection is available ([13741f51](https://github.com/hyperium/hyper/commit/13741f5145eb3dc894d2bc8d8486fc51c29e2e41))


### v0.11.20 (2018-02-26)


#### Bug Fixes

* **server:**
  * Make sleep_on_errors configurable and use it in example ([3a36eb55](https://github.com/hyperium/hyper/commit/3a36eb559676349d8a321c3159684503014f7fbe))
  * Sleep on socket IO errors ([68458cde](https://github.com/hyperium/hyper/commit/68458cde57a20f4b3c9c306eaf9801189262e0a6))


#### Features

* **body:** add `Body::is_empty()` method ([2f45d539](https://github.com/hyperium/hyper/commit/2f45d5394a2f8a49442ff4798a4b1651c079f0ff))
* **request:** add `Request::body_mut()` method ([3fa191a2](https://github.com/hyperium/hyper/commit/3fa191a2676feb86c91abf8dfcc8e63477980297))


### v0.11.19 (2018-02-21)


#### Bug Fixes

* **client:**
  * prevent empty bodies sending transfer-encoding for GET, HEAD ([77adab4e](https://github.com/hyperium/hyper/commit/77adab4ebf0fadd9ccd014d24ff0bcec1bce1e8b))
  * detect connection closes as pool tries to use ([dc619a8f](https://github.com/hyperium/hyper/commit/dc619a8fa01616b260ef32a35b35963460987206), closes [#1439](https://github.com/hyperium/hyper/issues/1439))
* **uri:** make absolute-form uris always have a path ([a9413d73](https://github.com/hyperium/hyper/commit/a9413d7367e8b9f0245fc8a90a22ece7d55e7e04))


#### Features

* **client:** Client will retry requests on fresh connections ([ee61ea9a](https://github.com/hyperium/hyper/commit/ee61ea9adf86b309490a68d044e40bd1090338e8))


### v0.11.18 (2018-02-07)


#### Bug Fixes

* **client:** send an `Error::Cancel` if a queued request is dropped ([88f01793](https://github.com/hyperium/hyper/commit/88f01793bec5830370cb88f74a64a2e20a440c17))


#### Features

* **client:** add `http1_writev` configuration option ([b0aa6497](https://github.com/hyperium/hyper/commit/b0aa6497258c20354ae0fe36d668e0c2361b3151))


### v0.11.17 (2018-02-05)


#### Bug Fixes

* **client:** more reliably detect closed pooled connections (#1434) ([265ad67c](https://github.com/hyperium/hyper/commit/265ad67c86379841a5aa821543a01648ccc8c26c))
* **h1:** fix hung streaming bodies over HTTPS ([73109694](https://github.com/hyperium/hyper/commit/731096947d0704de58b75d17e05af956bcb21bd9))


### v0.11.16 (2018-01-30)


#### Bug Fixes

* **client:**
  * check for dead connections in Pool ([44af2738](https://github.com/hyperium/hyper/commit/44af273853f82b81591b813d13627e143a14a6b7), closes [#1429](https://github.com/hyperium/hyper/issues/1429))
  * error on unsupport 101 responses, ignore other 1xx codes ([22774222](https://github.com/hyperium/hyper/commit/227742221fa7830a14c18becbbc6137d97b57729))
* **server:**
  * send 400 responses on parse errors before closing connection ([7cb72d20](https://github.com/hyperium/hyper/commit/7cb72d2019bffbc667b9ad2d8cbc19c1a513fcf7))
  * error if Response code is 1xx ([44c34ce9](https://github.com/hyperium/hyper/commit/44c34ce9adc888916bd67656cc54c35f7908f536))


#### Features

* **server:** add `Http::max_buf_size()` option ([d22deb65](https://github.com/hyperium/hyper/commit/d22deb6572c279e11773b6bcb862415c08f19c2e), closes [#1368](https://github.com/hyperium/hyper/issues/1368))
* **uri:** Add a `PartialEq<str>` impl for `Uri` ([11b49c2c](https://github.com/hyperium/hyper/commit/11b49c2cc84695e966e9d9a0b05781853b28d7a8))

#### Performance

- **h1:** utilize `writev` when possible, reducing copies ([68377ede](https://github.com/hyperium/hyper/commit/68377ede))

### v0.11.15 (2018-01-22)


#### Bug Fixes

* **lib:** properly handle HTTP/1.0 remotes ([36e66a50](https://github.com/hyperium/hyper/commit/36e66a50546347c6f9b74c6d3c26e8b910483a4b), closes [#1304](https://github.com/hyperium/hyper/issues/1304))


#### Features

* **client:** add `executor` method when configuring a `Client` ([c89019eb](https://github.com/hyperium/hyper/commit/c89019eb100d00b5235d3b9a0d0b672ab0ef8ddc))


### v0.11.14 (2018-01-16)


#### Bug Fixes

* **tokio-proto:** return end-of-body frame correctly for tokio-proto ([14e4c741](https://github.com/hyperium/hyper/commit/14e4c741dc48a386d7bdc6f8e9e279e60f172722), closes [#1414](https://github.com/hyperium/hyper/issues/1414))


### v0.11.13 (2018-01-12)


#### Bug Fixes

* **client:**
  * change connection errors to debug log level ([2fe90f25](https://github.com/hyperium/hyper/commit/2fe90f256420ff668966290ac96686ce061453e4), closes [#1412](https://github.com/hyperium/hyper/issues/1412))
  * don't error on read before writing request ([7976023b](https://github.com/hyperium/hyper/commit/7976023b594ec6784e40a147d3baec99a947b118))
* **lib:** properly handle body streaming errors ([7a48d0e8](https://github.com/hyperium/hyper/commit/7a48d0e8b4ad465c0205ddfb116b6bd60dbdec71))


### v0.11.12 (2018-01-08)


#### Bug Fixes

* **server:** add remote_addr back to Request when using Http::bind ([fa7f4377](https://github.com/hyperium/hyper/commit/fa7f4377c1d783ca860820aefc41d0eab73be14c), closes [#1410](https://github.com/hyperium/hyper/issues/1410))


### v0.11.11 (2018-01-05)


#### Features

* **client:** replace default dispatcher ([0892cb27](https://github.com/hyperium/hyper/commit/0892cb27777858737449a012bc6ea08ee080e5b7))
* **server:** change default dispatcher ([6ade21aa](https://github.com/hyperium/hyper/commit/6ade21aa7f16dfeb6c0c53fe39c3f168f5f8aec1))


### v0.11.10 (2017-12-26)


#### Bug Fixes

* **client:**
  * fix panic when request body is empty string ([bfb0f84d](https://github.com/hyperium/hyper/commit/bfb0f84d372ec4251a20d16a1ac514a4177e2a3b))
  * close connections when Response Future or Body is dropped ([ef400812](https://github.com/hyperium/hyper/commit/ef4008121e4faa9383fe4661ebd05de5efe7ee9c), closes [#1397](https://github.com/hyperium/hyper/issues/1397))
  * properly close idle connections after timeout ([139dc7ab](https://github.com/hyperium/hyper/commit/139dc7ab2be271cd58b909db16c6ddbe5109f133), closes [#1397](https://github.com/hyperium/hyper/issues/1397))
* **conn:** don't double shutdown in some cases ([7d3abfbc](https://github.com/hyperium/hyper/commit/7d3abfbcf33946cb8831103c3b55f9966fa9469d))


### v0.11.9 (2017-12-09)


#### Bug Fixes

* **client:** detect valid eof after reading a body ([15fdd53d](https://github.com/hyperium/hyper/commit/15fdd53d4cb1cd0fef41c4bed509020f44512a00), closes [#1396](https://github.com/hyperium/hyper/issues/1396))


#### Features

* **log:** improve quality of debug level logs ([7b593112](https://github.com/hyperium/hyper/commit/7b5931122a07f2a766d3e103001bcb5ee1f983f3))


### v0.11.8 (2017-12-06)


#### Bug Fixes

* **client:**
  * return error instead of unmatched response when idle ([95e0164e](https://github.com/hyperium/hyper/commit/95e0164e8f0f03742f71868cb2828bcd4bfa5cfc))
  * remove idle connections when read eof is found ([cecef9d4](https://github.com/hyperium/hyper/commit/cecef9d402b76af12e6415519deb2b604f77b195))
  * always wait on reads for pooled connections ([9f212410](https://github.com/hyperium/hyper/commit/9f212410026c780ea2a76ba81705ed137022260d))
  * don't leak connections with no keep-alive ([d2aa5d86](https://github.com/hyperium/hyper/commit/d2aa5d862c95168f4e71cc65155c2dc41f306f36), closes [#1383](https://github.com/hyperium/hyper/issues/1383))
* **conn:** handle when pre-emptive flushing closes the write state ([8f938d97](https://github.com/hyperium/hyper/commit/8f938d97e7f25ca9e8c9ae65f756f952753d9bf7), closes [#1391](https://github.com/hyperium/hyper/issues/1391))
* **lib:** fix `no_proto` dispatcher to flush queue before polling more body ([121b5eef](https://github.com/hyperium/hyper/commit/121b5eef19e65acfecb8261d865554e173f2fc78))
* **server:** allow TLS shutdown before dropping connections with `no_proto` ([60d0eaf8](https://github.com/hyperium/hyper/commit/60d0eaf8916f7cb5073105778f25dff21bd504bb), closes [#1380](https://github.com/hyperium/hyper/issues/1380))


#### Features

* **headers:** Implement `ProxyAuthorization` (#1394) ([c93cdb29](https://github.com/hyperium/hyper/commit/c93cdb290875cb86900e84c333725aefa4d7fad5))
* **server:**
  * Allow keep alive to be turned off for a connection (#1390) ([eb9590e3](https://github.com/hyperium/hyper/commit/eb9590e3da65299928938ae8bb830dfb008fdadd), closes [#1365](https://github.com/hyperium/hyper/issues/1365))
  * add `Http.serve_incoming` to wrap generic accept steams ([e4864a2b](https://github.com/hyperium/hyper/commit/e4864a2bea59b40fb07e6d18329f75817803a3f3))


### v0.11.7 (2017-11-14)


#### Bug Fixes

* **client:**
  * drop in-use connections when they finish if Client is dropped ([b1765dd1](https://github.com/hyperium/hyper/commit/b1765dd168b24912fbd36682f1f6df70eeb1acd5))
  * don't read extra bytes on idle connections ([7c4b814e](https://github.com/hyperium/hyper/commit/7c4b814e6b95bdb22b11e027b2da16c5abb8399f))
* **server:** GET requests with no body have None instead of Empty ([8bf79648](https://github.com/hyperium/hyper/commit/8bf7964875205155e3018902a6e8facee6c145b6), closes [#1373](https://github.com/hyperium/hyper/issues/1373))


#### Features

* **client:**
  * skip dns resolution when host is a valid ip addr ([b1785c66](https://github.com/hyperium/hyper/commit/b1785c662bc75f7bbd36a242c379d120ff7c6cd2))
  * allow custom executors for HttpConnector ([ed497bf5](https://github.com/hyperium/hyper/commit/ed497bf5e6f1d651e3b30fd42c10245c560aff5b))
  * add names to DNS threads ([e0de55da](https://github.com/hyperium/hyper/commit/e0de55daa2ec241f97fc5ed14f5ec933bde110d7))
* **header:** implement `ByteRangeSpec::to_satisfiable_range` ([bb54e36c](https://github.com/hyperium/hyper/commit/bb54e36c90dc9c2ca876cd7f2c7dc7250d217552))
* **lib:** add support to disable tokio-proto internals ([f7532b71](https://github.com/hyperium/hyper/commit/f7532b71d141ebe41172dbb863d58d519e387a4e))
* **server:**
  * add `const_service` and `service_fn` helpers ([fe38aa4b](https://github.com/hyperium/hyper/commit/fe38aa4bc1c8fdcaefb0d839239c14620a7b8f0a))
  * add `server::Serve` that can use a shared Handle ([39cf6ef7](https://github.com/hyperium/hyper/commit/39cf6ef7d26b3d829ec19fb1db176e8221170cb3))
  * allow creating Server with shared Handle ([0844dede](https://github.com/hyperium/hyper/commit/0844dede191d720e0336ee4aca63af2255abe458))


### v0.11.6 (2017-10-02)


#### Bug Fixes

* **server:** fix experimental pipeline flushing ([6b4635fd](https://github.com/hyperium/hyper/commit/6b4635fd13f5fe91ad6d388c5e66394627ad7ba2))


### v0.11.5 (2017-10-02)


#### Bug Fixes

* **http:** avoid infinite recursion when Body::from is called with Cow::Owned. (#1343) ([e8d61737](https://github.com/hyperium/hyper/commit/e8d6173734b0fb43bf7401fdbe43258d913a6284))


### v0.11.4 (2017-09-28)


#### Bug Fixes

* **client:**  fix panic in Pool ([0fbc215f](https://github.com/hyperium/hyper/commit/0fbc215f), closes [#1339](https://github.com/hyperium/hyper/issues/1339))


### v0.11.3 (2017-09-28)


#### Features

* **header:**  add ContentType::xml() constructor ([92595e84](https://github.com/hyperium/hyper/commit/92595e84))
* **http:**  add Body::from(cow) for bytes and strings ([425ff71d](https://github.com/hyperium/hyper/commit/425ff71d))
* **lib:**  implement compatibility with http crate ([0c7d375b](https://github.com/hyperium/hyper/commit/0c7d375b))
* **server:**
  *  add experimental pipeline flush aggregation option to Http ([dd54f20b](https://github.com/hyperium/hyper/commit/dd54f20b))
  *  remove unneeded Send + Sync from Server ([16e834d3](https://github.com/hyperium/hyper/commit/16e834d3))

#### Bug Fixes

* **client:**
  *  cleanup dropped pending Checkouts from Pool ([3b91fc65](https://github.com/hyperium/hyper/commit/3b91fc65), closes [#1315](https://github.com/hyperium/hyper/issues/1315))
  *  return Version errors if unsupported ([41c47241](https://github.com/hyperium/hyper/commit/41c47241), closes [#1283](https://github.com/hyperium/hyper/issues/1283))
* **http:**  log errors passed to tokio at debug level ([971864c4](https://github.com/hyperium/hyper/commit/971864c4), closes [#1278](https://github.com/hyperium/hyper/issues/1278))
* **lib:**
  *  Export hyper::RawStatus if the raw_status feature is enabled ([627c4e3d](https://github.com/hyperium/hyper/commit/627c4e3d))
  *  remove logs that contain request and response data ([207fca63](https://github.com/hyperium/hyper/commit/207fca63), closes [#1281](https://github.com/hyperium/hyper/issues/1281))

#### Performance

* **server:**  try to read from socket at keep-alive ([1a9f2648](https://github.com/hyperium/hyper/commit/1a9f2648))


### v0.11.2 (2017-07-27)


#### Bug Fixes

* **client:** don't assume bodies on 204 and 304 Responses ([81c0d185](https://github.com/hyperium/hyper/commit/81c0d185bdb2cb11e0fba231e3259097f492dd7d), closes [#1242](https://github.com/hyperium/hyper/issues/1242))
* **header:** fix panic from headers.remove when typed doesn't match ([4bd9746a](https://github.com/hyperium/hyper/commit/4bd9746a0fa56ddc578ec5a8044e6c37390f3770))
* **http:**
  * allow zero-length chunks when no body is allowed ([9b47e186](https://github.com/hyperium/hyper/commit/9b47e1861a6bd766f21c88b95ecfc9b45fad874d))
  * fix encoding when buffer is full ([fc5b9cce](https://github.com/hyperium/hyper/commit/fc5b9cce3176776e4c916cd1b907b1649a538f00))
  * skip zero length chunks when encoding ([d6da3f7b](https://github.com/hyperium/hyper/commit/d6da3f7b40550b425f760d0d331807feff9114fd))
* **server:**
  * improve detection of when a Response can have a body ([673e5cb1](https://github.com/hyperium/hyper/commit/673e5cb1a3dadea178e51677fa660a1258610ae8), closes [#1257](https://github.com/hyperium/hyper/issues/1257))
  * reject Requests with invalid body lengths ([14cbd400](https://github.com/hyperium/hyper/commit/14cbd40071816ec04dd1921e599c1d5cca883898))
  * do not automatically set ContentLength for 204 and 304 Responses ([c4c89a22](https://github.com/hyperium/hyper/commit/c4c89a22f8f1ebc74a13a6ee75a8209081dcb535))
* **uri:** fix Uri parsing of IPv6 and userinfo ([7081c449](https://github.com/hyperium/hyper/commit/7081c4498e707c1240c7e672d39ba4948fffb558), closes [#1269](https://github.com/hyperium/hyper/issues/1269))


#### Features

* **headers:** export missing header types ([c9f4ff33](https://github.com/hyperium/hyper/commit/c9f4ff33821df1bff557dfddac1ba3fc6255ee62))
* **server:** Provide reference to Response body ([a79fc98e](https://github.com/hyperium/hyper/commit/a79fc98e36eac485803b1ab97f35c60198fd72cb), closes [#1216](https://github.com/hyperium/hyper/issues/1216))
* **status:** add `as_u16()` method to `StatusCode` ([5f6f252c](https://github.com/hyperium/hyper/commit/5f6f252c603c642be8037682c1bf7e7ed2392a53))


### v0.11.1 (2017-07-03)


#### Features

* **server:** Handle 100-continue ([6164e764](https://github.com/hyperium/hyper/commit/6164e76405935065aeb912f94ba94230e0bac60f))


## v0.11.0 (2017-06-13)

#### Bug Fixes

* **header:**
  * add length checks to `ETag` parsing ([643fac1e](https://github.com/hyperium/hyper/commit/643fac1e01102524e44ead188e865830ebdfb1f4))
  * prevent 2 panics in `QualityItem` parsing ([d80aae55](https://github.com/hyperium/hyper/commit/d80aae55b1af0420bfcdecb2c8515b48e3e0e641))
  * Allow IPv6 Addresses in `Host` header ([8541ac72](https://github.com/hyperium/hyper/commit/8541ac72d7ec80a36171115501e49dd47bcb1d0d))
  * Remove raw part when getting mutable reference to typed header ([f38717e4](https://github.com/hyperium/hyper/commit/f38717e422a80e04ca95fcd5e5c5d54b7197bed2), closes [#821](https://github.com/hyperium/hyper/issues/821))
  * only add chunked to `TransferEncoding` if not present ([1b4f8579](https://github.com/hyperium/hyper/commit/1b4f85799737a537d8ebfb6afd0423b97238ab8b))
  * ignore invalid cookies ([310d98d5](https://github.com/hyperium/hyper/commit/310d98d50b929b8bde898cbb1137df95da5e0840))
* **http:**
  * Chunked decoder reads last `\r\n` ([bffde8c8](https://github.com/hyperium/hyper/commit/bffde8c841353e05e9aea267ca94848ccdeeb394))
  * make Chunked decoder resilient in an async world ([8672ec5a](https://github.com/hyperium/hyper/commit/8672ec5a366e698bd32679d64dce925b3fa11fc6))
* **server:**
  * support HTTP/1.1 pipelining ([523b890a](https://github.com/hyperium/hyper/commit/523b890a19e9325938adf42456eea6191fcb8029))

#### Features

* **body:**
  * implement Extend and IntoIterator for Chunk ([78512bdb](https://github.com/hyperium/hyper/commit/78512bdb184903061ea02f1101c99a097483cb69))
  * add Default trait to Body ([f61708ba](https://github.com/hyperium/hyper/commit/f61708ba81fc03a4797688afd5bcec87e8f98eef))
  * implement `Default` for `Body` ([6faa653f](https://github.com/hyperium/hyper/commit/6faa653f0dfaa5220e76a60fcd264511686dfd08))
  * implement `Default` for `Chunk` ([f5567db4](https://github.com/hyperium/hyper/commit/f5567db4dcc04a769725d0b9ccb6a81bc3026acc))
* **client:**
  * add `HttpConnector.enforce_http` ([1c34a05a](https://github.com/hyperium/hyper/commit/1c34a05a85078421078f2cb266dccc5dfce8a9f0))
  * add an accessor for the request body ([4e26646a](https://github.com/hyperium/hyper/commit/4e26646aa7b46d5739d3978126bb70e8c47cde1d))
  * Response.status() now returns a `StatusCode` by value ([d63b7de4](https://github.com/hyperium/hyper/commit/d63b7de44f813696f8ec595d2f8f901526c1720e))
  * add Client::handle ([9101817b](https://github.com/hyperium/hyper/commit/9101817b0fd61d7bcccfaa8933e64d6e3787395d))
  * add Request.set_proxy for HTTP proxy requests ([e8714116](https://github.com/hyperium/hyper/commit/e871411627cab5caf00d8ee65328da9ff05fc53d), closes [#1056](https://github.com/hyperium/hyper/issues/1056))
  * DNS worker count is configurable ([138e1643](https://github.com/hyperium/hyper/commit/138e1643e81669cae9dbe215197abd0e07f0c1e7))
  * add keep_alive_timeout to Client ([976218ba](https://github.com/hyperium/hyper/commit/976218badc4a067e45a9d15af7e4eb5f2a4adc09))
* **error:** Display for Error shows better info ([49e196db](https://github.com/hyperium/hyper/commit/49e196db1c91b2fb5f7ab05d99b9c7bc997195f2), closes [#694](https://github.com/hyperium/hyper/issues/694))
* **header:**
  * add ContentType::octet_stream() constructor ([1a353102](https://github.com/hyperium/hyper/commit/1a35310273732acbf8e8498ebb5dbad3d61386cb))
  * change `Cookie` to be map-like ([dd03e723](https://github.com/hyperium/hyper/commit/dd03e7239238e6c0753cf2502a0534e2c9770d9e), closes [#1145](https://github.com/hyperium/hyper/issues/1145))
  * add `Cookie::iter()` ([edc1c0dd](https://github.com/hyperium/hyper/commit/edc1c0dd01b24ee32250dff51268ad60fff9293d))
  * implement fmt::Display for several headers ([e9e7381e](https://github.com/hyperium/hyper/commit/e9e7381ece21588076bb712d5c508f50cd740591))
  * add `Headers::append_raw` ([b4b2fb78](https://github.com/hyperium/hyper/commit/b4b2fb782e51b2b932e52fab6add7c23a369f1fb))
  * Add support for Retry-After header ([1037bc77](https://github.com/hyperium/hyper/commit/1037bc773256ca05c4311a781e96fbdcaac877fe))
  * add `Encoding::Brotli` variant ([f0ab2b6a](https://github.com/hyperium/hyper/commit/f0ab2b6aedb909d37698365d1fcc34ce749304b5))
  * introduce `header::Raw` (#869) ([50ccdaa7](https://github.com/hyperium/hyper/commit/50ccdaa7e7db574ec9890c220765ffd2da5e493b))
  * add `TE` header struct (#1150) ([f1859dfd](https://github.com/hyperium/hyper/commit/f1859dfd7abfc124dd986edc413f754f76c76e8b), closes [#1109](https://github.com/hyperium/hyper/issues/1109))
  * support Opaque origin headers (#1147) ([41485997](https://github.com/hyperium/hyper/commit/414859978b47dc8ebd0df264afc4e113b8a1909e), closes [#1065](https://github.com/hyperium/hyper/issues/1065))
  * add `HeaderView.raw()` ([8143c33b](https://github.com/hyperium/hyper/commit/8143c33bad9146414f14197c39f6d5326d0f0212))
  * `impl Eq for ContentType` ([bba761ac](https://github.com/hyperium/hyper/commit/bba761ac547b59c885aceea5b9e52bf52e8747b5))
  * add `Link` header implementation ([592c1e21](https://github.com/hyperium/hyper/commit/592c1e21256d3ba2aeba6cdc2b62d8c1ebfa1dbf), closes [#650](https://github.com/hyperium/hyper/issues/650))
  * add `star`, `json`, `text`, `image` constructors to `Accept` ([bdc19d52](https://github.com/hyperium/hyper/commit/bdc19d52bf5ec2e63b785de31bfe0ad3ba4d2550))
  * Add strict-origin and strict-origin-when-cross-origin referer policy ([3593d798](https://github.com/hyperium/hyper/commit/3593d7987a92518736e130586499d97afa3e5b04))
  * support multiple values for Referrer-Policy header ([7b558ae8](https://github.com/hyperium/hyper/commit/7b558ae87a826ca7383c0034d4ca95fc61aeac4c), closes [#882](https://github.com/hyperium/hyper/issues/882))
  * add `Warning` header ([69894d19](https://github.com/hyperium/hyper/commit/69894d19947f01ad4ff54ce0283429758acba9ff), closes [#883](https://github.com/hyperium/hyper/issues/883))
  * `Headers::remove` returns the Header ([9375addb](https://github.com/hyperium/hyper/commit/9375addba03505f2515d493364f9b1beb8b9b99a), closes [#891](https://github.com/hyperium/hyper/issues/891))
  * add `ContentLocation` header ([13c5bf66](https://github.com/hyperium/hyper/commit/13c5bf66c305c08a2a1af26e48115b667d141b18), closes [#870](https://github.com/hyperium/hyper/issues/870))
  * add `LastEventId` header ([e1542a60](https://github.com/hyperium/hyper/commit/e1542a609f99da770a65500333d922c58e39d179))
  * add `Origin header ([01843f88](https://github.com/hyperium/hyper/commit/01843f882265a894c7051dc2ecf5cf09f2c2e8e7), closes [#651](https://github.com/hyperium/hyper/issues/651))
  * Add `ReferrerPolicy` header ([3a86b3a2](https://github.com/hyperium/hyper/commit/3a86b3a2b25be1c088cf7d39bb431b2e624d4191))
* **http:**
  * add Into<Bytes> for Chunk ([fac3d70c](https://github.com/hyperium/hyper/commit/fac3d70c0b716157ba689ae2b8a0089b6afc9bdc))
  * use the bytes crate for Chunk and internally ([65b3e08f](https://github.com/hyperium/hyper/commit/65b3e08f6904634294ff2d105f2551cafe7e754d))
  * allow specifying custom body streams ([1b1311a7](https://github.com/hyperium/hyper/commit/1b1311a7d36b000c9c2c509971ee759da8765711))
* **lib:**
  * add `raw_status` feature in Cargo.toml ([acd62cda](https://github.com/hyperium/hyper/commit/acd62cda446e4c647716a2d595342360dc24a080))
  * remove extern Url type usage ([4fb7e6eb](https://github.com/hyperium/hyper/commit/4fb7e6ebc6b1d429dcce4bc18139bd443fffa6ee))
  * export Method::Put at top level ([5c890321](https://github.com/hyperium/hyper/commit/5c890321ee2da727a814c18d4ee2df5eddd6720e))
  * redesign API to use Futures and Tokio ([2d2d5574](https://github.com/hyperium/hyper/commit/2d2d5574a698e74e5102d39b9a9ab750860d92d1))
  * switch to non-blocking (asynchronous) IO ([d35992d0](https://github.com/hyperium/hyper/commit/d35992d0198d733c251e133ecc35f2bca8540d96))
* **mime:** upgrade to mime v0.3 ([f273224f](https://github.com/hyperium/hyper/commit/f273224f21eedd2f466f12fe30fd24e83c35922c), closes [#738](https://github.com/hyperium/hyper/issues/738))
* **server:**
  * make Http default its body type to hyper::Chunk ([dc97dd77](https://github.com/hyperium/hyper/commit/dc97dd77f45486d9cb9a22a1859809c5af5579e2))
  * make Http compatible with TcpServer ([e04bcc12](https://github.com/hyperium/hyper/commit/e04bcc12a7e081f75482cdca1e4f4c4f597ad2ce), closes [#1036](https://github.com/hyperium/hyper/issues/1036))
  * add path() and query() to Request ([8b3c1206](https://github.com/hyperium/hyper/commit/8b3c1206846cb96be780923952eafe0dde7850bf), closes [#896](https://github.com/hyperium/hyper/issues/896), [#897](https://github.com/hyperium/hyper/issues/897))
* **status:**
  * add `StatusCode::try_from(u16)`. ([f953cafe](https://github.com/hyperium/hyper/commit/f953cafe27d1c5de0c8b859e485225cfc2c18629))
  * remove deprecated `StatusClass` ([94ee6204](https://github.com/hyperium/hyper/commit/94ee6204ae32b8c431c00fdc03dc75eee573c69c))
  * impl Into<u16> for StatusCode ([c42f18db](https://github.com/hyperium/hyper/commit/c42f18db05e47fc24e8a8ece76cbc782b7558e8b))
* **uri:**
  * redesign `RequestUri` type into `Uri` ([9036443e](https://github.com/hyperium/hyper/commit/9036443e6bd61b948ebe622588d2765e22e2b179), closes [#1000](https://github.com/hyperium/hyper/issues/1000))
  * add `is_absolute` method to `Uri` ([154ab29c](https://github.com/hyperium/hyper/commit/154ab29c0d2b50d7bcac0f7918abf2f7a1628112))
* **version:** impl `FromStr` for `HttpVersion` ([47f3aa62](https://github.com/hyperium/hyper/commit/47f3aa6247a3211ae499b30584dca6acb43d2204))


#### Breaking Changes

* The `Cookie` header is no longer a wrapper over a
  `Vec<String>`. It must be accessed via its `get` and `set` methods.

 ([dd03e723](https://github.com/hyperium/hyper/commit/dd03e7239238e6c0753cf2502a0534e2c9770d9e))
* Any use of `Quality(num)` should change to `q(num)`.

 ([a4644959](https://github.com/hyperium/hyper/commit/a4644959b0f980d94898d6c2e3cb1763aac73a5e))
* `HttpDate` no longer has public fields. Convert between
  `HttpDate` and `SystemTime` as needed.

 ([316c6fad](https://github.com/hyperium/hyper/commit/316c6fad3026ba5ff5f6b9f31aca4d4f74b144e0))
* The `link_extensions` methods of the `Link` header are
  removed until fixed.

 ([011f28cb](https://github.com/hyperium/hyper/commit/011f28cb18d285401bc8bea2b0f0dbdf80089d97))
* The `fmt_header` method has changed to take a different
  formatter. In most cases, if your header also implements
  `fmt::Display`, you can just call `f.fmt_line(self)`.

 ([6f02d43a](https://github.com/hyperium/hyper/commit/6f02d43ae0d80971a32617e316498b81acf38ca2))
* The `Encoding` enum has an additional variant, `Trailers`.

 ([f1859dfd](https://github.com/hyperium/hyper/commit/f1859dfd7abfc124dd986edc413f754f76c76e8b))
* `Origin.scheme` and `Origin.host` now return `Option`s, since the `Origin` could be `null`.

 ([41485997](https://github.com/hyperium/hyper/commit/414859978b47dc8ebd0df264afc4e113b8a1909e))
* If you were explicitly checking the `StatusCode`, such as
  with an equality comparison, you will need to use the value instead of a
  reference.

 ([d63b7de4](https://github.com/hyperium/hyper/commit/d63b7de44f813696f8ec595d2f8f901526c1720e))
* This removes several deprecated methods for converting
  Headers into strings. Use more specialized methods instead.

 ([ec91bf41](https://github.com/hyperium/hyper/commit/ec91bf418b1f285bac9231d4bee0dd96742e565a))
* The `Url` type is no longer used. Any instance in the
  `Client` API has had it replaced with `hyper::Uri`.

  This also means `Error::Uri` has changed types to
  `hyper::error::UriError`.

  The type `hyper::header::parsing::HTTP_VALUE` has been made private,
  as an implementation detail. The function `http_percent_encoding`
  should be used instead.

 ([4fb7e6eb](https://github.com/hyperium/hyper/commit/4fb7e6ebc6b1d429dcce4bc18139bd443fffa6ee))
* This makes `Request.remote_addr` an
  `Option<SocketAddr>`, instead of `SocketAddr`.

 ([e04bcc12](https://github.com/hyperium/hyper/commit/e04bcc12a7e081f75482cdca1e4f4c4f597ad2ce))
* The `Preference` header had a typo in a variant and it's string representation,
  change `Preference::HandlingLeniant` to `Preference::HandlingLenient`.
 ([2fa414fb](https://github.com/hyperium/hyper/commit/2fa414fb5fe6dbc922da25cca9960652edf32591))
* `Server` is no longer the primary entry point. Instead,
  an `Http` type is created  and then either `bind` to receive a `Server`,
  or it can be passed to other Tokio things.
 ([f45e9c8e](https://github.com/hyperium/hyper/commit/f45e9c8e4fcacc2bd7fed84ef0df6d2fcf8c1134))
* The name of `RequestUri` has changed to `Uri`. It is no
  longer an `enum`, but an opaque struct with getter methods.

 ([9036443e](https://github.com/hyperium/hyper/commit/9036443e6bd61b948ebe622588d2765e22e2b179))
* This adds a new variant to the `Encoding` enum, which
  can break exhaustive matches.

 ([f0ab2b6a](https://github.com/hyperium/hyper/commit/f0ab2b6aedb909d37698365d1fcc34ce749304b5))
* The fields of the `Host` header are no longer
  available. Use the getter methods instead.

 ([cd9fd522](https://github.com/hyperium/hyper/commit/cd9fd522074bfe530c30c878e49e6ac1bd881f1f))
* A big sweeping set of breaking changes.

 ([2d2d5574](https://github.com/hyperium/hyper/commit/2d2d5574a698e74e5102d39b9a9ab750860d92d1))
* `Headers.remove()` used to return a `bool`,
  it now returns `Option<H>`. To determine if a a header exists,
  switch to `Headers.has()`.
 ([9375addb](https://github.com/hyperium/hyper/commit/9375addba03505f2515d493364f9b1beb8b9b99a))
* `Header::parse_header` now receives `&Raw`, instead of
  a `&[Vec<u8>]`. `Raw` provides several methods to ease using it, but
  may require some changes to existing code.
 ([50ccdaa7](https://github.com/hyperium/hyper/commit/50ccdaa7e7db574ec9890c220765ffd2da5e493b))
* LanguageTag used to be at the crate root, but it is now
  in the `hyper::header` module.

 ([40745c56](https://github.com/hyperium/hyper/commit/40745c5671daf8ac7eb342ff0e1e7c801a7171c4))
* Removes the undocumented `from_u16` function. Use
  `StatusCode::try_from` instead.

  Also makes the `status` module private. All imports of
  `hyper::status::StatusCode` should be `hyper::StatusCode`.

 ([f953cafe](https://github.com/hyperium/hyper/commit/f953cafe27d1c5de0c8b859e485225cfc2c18629))
* All usage of `status.class()` should change to
  equivalent `status.is_*()` methods.

 ([94ee6204](https://github.com/hyperium/hyper/commit/94ee6204ae32b8c431c00fdc03dc75eee573c69c))
* Most uses of `mime` will likely break. There is no more
  `mime!` macro, nor a `Mime` constructor, nor `TopLevel` and `SubLevel`
  enums.

  Instead, in most cases, a constant exists that can now be used.

  For less common mime types, they can be created by parsing a string.

 ([f273224f](https://github.com/hyperium/hyper/commit/f273224f21eedd2f466f12fe30fd24e83c35922c))
* To use `RawStatus`, you must enable the `raw_status`
  crate feature.

 ([acd62cda](https://github.com/hyperium/hyper/commit/acd62cda446e4c647716a2d595342360dc24a080))
* Some headers used `UniCase`, but now use
  `unicase::Ascii`. Upgrade code to `Ascii::new(s)`.

 ([c81edd41](https://github.com/hyperium/hyper/commit/c81edd41d783f67eca7a50d83b40c8a7cedf333c))
* This breaks a lot of the Client and Server APIs.
  Check the documentation for how Handlers can be used for asynchronous
  events.

 ([d35992d0](https://github.com/hyperium/hyper/commit/d35992d0198d733c251e133ecc35f2bca8540d96))


### v0.10.9 (2017-04-19)


#### Features

* **server:** add local_addr to retrieve resolved address ([71f250ad](https://github.com/hyperium/hyper/commit/71f250ad46e9ae0cac108e1de6dc15289da26a56))


### v0.10.8 (2017-04-11)


#### Features

* **client:**
  * introduce `PooledStream::<S>::get_ref` ([a54ce30f](https://github.com/hyperium/hyper/commit/a54ce30f902772168bbd8dc90f26bb08cecde6ec))
  * introduce Response::get_ref ([5ef0ec2c](https://github.com/hyperium/hyper/commit/5ef0ec2cd2841e78508a61949a207187be914265))


### v0.10.7 (2017-04-08)


#### Bug Fixes

* **server:** don't dup the listener TCP socket. ([d2362331](https://github.com/hyperium/hyper/commit/d23623317820696c910ce43262d5276e8e24c066))


### v0.10.6 (2017-04-05)


#### Features

* **buffer:** add from_parts and into_parts functions ([78551dd0](https://github.com/hyperium/hyper/commit/78551dd040e2ab46e833af355c92fe87aa026244))


### v0.10.5 (2017-03-01)


#### Bug Fixes

* **http:**
  * Consume entire chunked encoding message ([4147fcd0](https://github.com/hyperium/hyper/commit/4147fcd0d688b6d5b8d6b32f26c147819321a390))
  * fix returning EarlyEof if supplied buffer is zero-len ([1e740fbc](https://github.com/hyperium/hyper/commit/1e740fbcc3fb60af2fe8d2227457fa29582151c3))


### v0.10.4 (2017-01-31)


#### Features

* **header:** implement fmt::Display for several headers ([d5075770](https://github.com/hyperium/hyper/commit/d50757707b1c628f398fb0583aa3dd02111ae658))


### v0.10.3 (2017-01-30)


#### Bug Fixes

* **header:**
  * deprecate HeaderFormatter ([282466e1](https://github.com/hyperium/hyper/commit/282466e1c00879cf9dde1ed62c3d436e99bfba85))
  * enable SetCookie.fmt_header when only 1 cookie ([7611c307](https://github.com/hyperium/hyper/commit/7611c3071475afa2b0b80bbba2a0a7223a3d5920))


#### Features

* **header:** add Headers::append_raw ([6babbc40](https://github.com/hyperium/hyper/commit/6babbc40fb86a29ad76083a2a386182c40c0f335))


### v0.10.2 (2017-01-23)


#### Bug Fixes

* **header:** security fix for header values that include newlines ([2603d78f](https://github.com/hyperium/hyper/commit/2603d78f59d284953553b7ef48c3ea4baa085cd1))
* **net:** set timeouts directly in `accept` ([f5d4d653](https://github.com/hyperium/hyper/commit/f5d4d653e35ed20bbbb0b13847b3b9f1cfe9575f))


#### Breaking Changes

* This technically will cause code that a calls
  `SetCookie.fmt_header` to panic, as it is no longer to properly write
  that method. Most people should not be doing this at all, and all
  other ways of printing headers should work just fine.

  The breaking change must occur in a patch version because of the
  security nature of the fix.

 ([2603d78f](https://github.com/hyperium/hyper/commit/2603d78f59d284953553b7ef48c3ea4baa085cd1))


### v0.10.1 (2017-01-19)


## v0.10.0 (2017-01-10)

#### Features

* **client:**
  * change ProxyConfig to allow HTTPS proxies ([14a4f1c2](https://github.com/hyperium/hyper/commit/14a4f1c2f735efe7b638e9078710ca32dc1e360a))
  * remove experimental HTTP2 support ([d301c6a1](https://github.com/hyperium/hyper/commit/d301c6a1708c7d408b7f03ac46674a5f0edd3253))
* **header:** remove `cookie` dependency ([f22701f7](https://github.com/hyperium/hyper/commit/f22701f7e7258ad4a26645eba47a3d374e452e86))
* **lib:**
  * remove SSL dependencies ([2f48612c](https://github.com/hyperium/hyper/commit/2f48612c7e141a9d612d7cb9d524b2f460561f56))
  * remove `serde-serialization` feature ([7b9817ed](https://github.com/hyperium/hyper/commit/7b9817edcf4451bd033e55467c75577031bfe740))


#### Breaking Changes

* There is no more `hyper::http::h2`.

  ([d301c6a1](https://github.com/hyperium/hyper/commit/d301c6a1708c7d408b7f03ac46674a5f0edd3253))
* The `Cookie` and `SetCookie` headers no longer use the
  cookie crate. New headers can be written for any header, or the ones
  provided in hyper can be accessed as strings.

  ([f22701f7](https://github.com/hyperium/hyper/commit/f22701f7e7258ad4a26645eba47a3d374e452e86))
* There is no longer a `serde-serialization` feature.
  Look at external crates, like `hyper-serde`, to fulfill this feature.

  ([7b9817ed](https://github.com/hyperium/hyper/commit/7b9817edcf4451bd033e55467c75577031bfe740))
* hyper will no longer provide OpenSSL support out of the
  box. The `hyper::net::Openssl` and related types are gone. The `Client`
  now uses an `HttpConnector` by default, which will error trying to
  access HTTPS URLs.

  TLS support should be added in from other crates, such as
  hyper-openssl, or similar using different TLS implementations.

  ([2f48612c](https://github.com/hyperium/hyper/commit/2f48612c7e141a9d612d7cb9d524b2f460561f56))
* Usage of `with_proxy_config` will need to change to
  provide a network connector. For the same functionality, a
  `hyper::net::HttpConnector` can be easily created and passed.

  ([14a4f1c2](https://github.com/hyperium/hyper/commit/14a4f1c2f735efe7b638e9078710ca32dc1e360a))


### v0.9.14 (2016-12-12)


#### Features

* **headers:** add star, json, text, image constructors to Accept ([a9fbbd7f](https://github.com/hyperium/hyper/commit/a9fbbd7fdbcbec51ef560e9882a8fefa64a93b54))
* **server:** add 'take_buf' method to BufReader ([bbbce5fc](https://github.com/hyperium/hyper/commit/bbbce5fc8bca0bcc34df4a4a9223432085fba2ff))


### v0.9.13 (2016-11-27)


#### Bug Fixes

* **client:** close Pooled streams on sockopt error ([d5ffee2e](https://github.com/hyperium/hyper/commit/d5ffee2ec801274ac271273289084b7251b4ce89))


### v0.9.12 (2016-11-09)


#### Features

* **error:** re-export url::ParseError ([30e78ac2](https://github.com/hyperium/hyper/commit/30e78ac212ed3085a5217e8d7f641c2f161ddc87))


### v0.9.11 (2016-10-31)


#### Bug Fixes

* **headers:** Allow IPv6 Addresses in Host header ([20f177ab](https://github.com/hyperium/hyper/commit/20f177abec12397f23adf43f6b726daee1a731cf))


#### Features

* **headers:**
  * Add strict-origin and strict-origin-when-cross-origin referer policy ([1be4e769](https://github.com/hyperium/hyper/commit/1be4e7693f7d27c049f35fefb9fffead2581b1f4))
  * support multiple values for Referrer-Policy header ([dc476657](https://github.com/hyperium/hyper/commit/dc4766573af9bd31d57fede5b9ef0ffa56fe44ab), closes [#882](https://github.com/hyperium/hyper/issues/882))
  * add last-event-id header ([2277987f](https://github.com/hyperium/hyper/commit/2277987f3c25380353db606ca7baaf0c854095cd))
* **server:** accept combined certificate files ([eeb1f48e](https://github.com/hyperium/hyper/commit/eeb1f48e17f4c71162ce90f88bda3dc37b489cc7))


### v0.9.10 (2016-07-11)


#### Features

* **headers:**
  * add origin header ([64881ae0](https://github.com/hyperium/hyper/commit/64881ae05458f06261b2e7d0f790184678cc42b9))
  * Add Referrer-Policy header ([b76a02cc](https://github.com/hyperium/hyper/commit/b76a02cc446f2a3935006035fd73f5f7a47ec428))


### v0.9.9 (2016-06-21)


#### Bug Fixes

* **headers:** Remove raw part when getting mutable reference to typed header ([63b61524](https://github.com/hyperium/hyper/commit/63b615249443b8f897018f21473c2f1f8e43663c), closes [#821](https://github.com/hyperium/hyper/issues/821))


#### Features

* **error:** Display for Error shows better info ([5620fbf9](https://github.com/hyperium/hyper/commit/5620fbf98f1fd43482a9ffa3c98f2f38b42fd4b0), closes [#694](https://github.com/hyperium/hyper/issues/694))


### v0.9.8 (2016-06-14)


#### Features

* **client:** enable use of custom TLS wrapper for proxied connections ([0476196c](https://github.com/hyperium/hyper/commit/0476196c320765a66f730c56048998980b173caf), closes [#824](https://github.com/hyperium/hyper/issues/824))


### v0.9.7 (2016-06-09)


#### Bug Fixes

* **proxy:** fix the 0.9.x build with `--no-default-features --features=security-framework` ([6caffe9f](https://github.com/hyperium/hyper/commit/6caffe9fb302da99ce8cf0c8027c06b8c6de782d), closes [#819](https://github.com/hyperium/hyper/issues/819))
* **server:** Request.ssl() works ([ce0b62ea](https://github.com/hyperium/hyper/commit/ce0b62eae7688987b722599be8e8b2ff6764b224))


### v0.9.6 (2016-05-23)


#### Bug Fixes

* **client:** Manually impl Debug for PooledStream ([aa692236](https://github.com/hyperium/hyper/commit/aa692236a851d29abec63b6a0d61d957cea5fd26))
* **server:** Switch Ssl to SslServer in bounds ([470bc8ec](https://github.com/hyperium/hyper/commit/470bc8ec396bfc9ead6782f72e6de58268767a5a))


### v0.9.5 (2016-05-18)


#### Bug Fixes

* **windows:** disable openssl cert validation for Windows ([c89aca81](https://github.com/hyperium/hyper/commit/c89aca812bf863aadb52326f534a65c1d3cf31d6), closes [#794](https://github.com/hyperium/hyper/issues/794))


#### Features

* **net:** Add OpensslClient constructor ([3c0e1050](https://github.com/hyperium/hyper/commit/3c0e105011fc8a4fc639370836aa6a2e576b6f0e))


### v0.9.4 (2016-05-09)


#### Bug Fixes

* **warnings:** remove unused_code warnings from newest nightlies ([e7229480](https://github.com/hyperium/hyper/commit/e7229480ea669bbe62fe644e312ba06cdca45b1c))


#### Features

* **ssl:**
  * enable hostname verification by default for OpenSSL ([01160abd](https://github.com/hyperium/hyper/commit/01160abd92956e5f995cc45790df7a2b86c8989f), closes [#472](https://github.com/hyperium/hyper/issues/472))
  * use secure ciphers by default in openssl ([54bf6ade](https://github.com/hyperium/hyper/commit/54bf6adeee1c3a231925f3efa7e38f875bc2d4d5))

### v0.9.3 (2016-05-09)


#### Bug Fixes

* **client:** fix panic in Pool::connect ([e51bafe2](https://github.com/hyperium/hyper/commit/e51bafe2e4f2a1efc36790232bef488c91131d0b), closes [#780](https://github.com/hyperium/hyper/issues/780))


### v0.9.2 (2016-05-04)


#### Features

* **client:**
  *  proper proxy and tunneling in Client ([f36c6b25](https://github.com/hyperium/hyper/commit/f36c6b25), closes [#774](https://github.com/hyperium/hyper/issues/774))
  *  add Proxy support ([25010fc1](https://github.com/hyperium/hyper/commit/25010fc1), closes [#531](https://github.com/hyperium/hyper/issues/531))

#### Performance

* **client:**  don't keep Pool mutex locked during getaddrinfo ([5fcc04a6](https://github.com/hyperium/hyper/commit/5fcc04a6))


### v0.9.1 (2016-04-21)


#### Bug Fixes

* **Cargo.toml:** update documentation link ([b783ddf4](https://github.com/hyperium/hyper/commit/b783ddf455ee93cc38510f3179ffe18733c797c1))


## v0.9.0 (2016-04-21)


#### Features

* **net:** Add Ssl impls for security-framework ([f37315b2](https://github.com/hyperium/hyper/commit/f37315b2708e092eaf5177a6960df9f7bf11eb5c))


#### Breaking Changes

* The re-exported Url type has breaking changes.
 ([8fa7a989](https://github.com/hyperium/hyper/commit/8fa7a9896809ef2a24994993b91981105a520f26))


### v0.8.1 (2016-04-13)


#### Bug Fixes

* **headers:** correctly handle repeated headers ([70c69142](https://github.com/hyperium/hyper/commit/70c6914217a9b48880e61b7fb59acd15c6e1421e), closes [#683](https://github.com/hyperium/hyper/issues/683))


#### Features

* **header:** add prefer and preference applied headers ([6f649301](https://github.com/hyperium/hyper/commit/6f6493010a9c190b29aceb3c10c65785923a85f5), closes [#747](https://github.com/hyperium/hyper/issues/747))
* **net:** Split Ssl into SslClient and SslServer ([2c86e807](https://github.com/hyperium/hyper/commit/2c86e8078ec01db2283e1fee1461db4c7bf76d3f), closes [#756](https://github.com/hyperium/hyper/issues/756))


## v0.8.0 (2016-03-14)


#### Bug Fixes

* **headers:** remove charset from `ContentType::json()` convenience method ([ec568e9a](https://github.com/hyperium/hyper/commit/ec568e9a551018b3353b6754eb2fcd729c7ea3c6))
* **net:** fix the typo in `set_write_timeout` ([7c76fff3](https://github.com/hyperium/hyper/commit/7c76fff3aaf0f0a300e76622acb56eaf1e2cb474))


#### Features

* **client:** Implement Debug for Client ([8c7ef7fd](https://github.com/hyperium/hyper/commit/8c7ef7fd937616798780d43f80a6b46507bc3433))
* **status:** add HTTP statuses 421 and 451 ([93fd5a87](https://github.com/hyperium/hyper/commit/93fd5a87bddc5bfe29f35f86d44d3f46c81ff5fa))


#### Breaking Changes

* mime 0.2 depends on serde 0.7, so any instances of
  using older versions of serde will need to upgrade.

 ([146df53c](https://github.com/hyperium/hyper/commit/146df53caf2a70cd15f97710738ba8d350040c12))


### v0.7.2 (2016-01-04)


#### Bug Fixes

* **buffer:** fix incorrect resizing of BufReader ([3a18e72b](https://github.com/hyperium/hyper/commit/3a18e72be67152834f6967c6d208f214288178ee), closes [#715](https://github.com/hyperium/hyper/issues/715))


#### Features

* **headers:** allow ExtendedValue structs to be formatted and used as struct members ([da0abe89](https://github.com/hyperium/hyper/commit/da0abe8988a61281b447a554b65ea8fd5d54f81b))


### v0.7.1 (2015-12-19)


#### Bug Fixes

* **cargo:** remove * dependencies for serde and env_logger ([4a05bee9](https://github.com/hyperium/hyper/commit/4a05bee9abdc426bbd904fe356b771e492dc8f43))
* **server:**
  * Flush 100-continue messages ([92ff50f2](https://github.com/hyperium/hyper/commit/92ff50f2e57fa2cb8a55b3d6d9fa43ef9a1b5526), closes [#704](https://github.com/hyperium/hyper/issues/704))
  * Removed check for GET/HEAD request when parsing body ([0b05c590](https://github.com/hyperium/hyper/commit/0b05c5903e86327cc9cb4cac39217e496851fce3), closes [#698](https://github.com/hyperium/hyper/issues/698))


#### Features

* **headers:** add extended parameter parser to the public API ([402fb76b](https://github.com/hyperium/hyper/commit/402fb76bb2f3dab101509e4703743ab075ae41be))


## v0.7.0 (2015-11-24)


#### Features

* **all:** add socket timeouts ([fec6e3e8](https://github.com/hyperium/hyper/commit/fec6e3e873eb79bd17d1c072d2ca3c7b91624f9c))
* **headers:**
  * Add Content-Disposition header ([7623ecc2](https://github.com/hyperium/hyper/commit/7623ecc26466e2e072eb2b03afc5e6c16d8e9bc9), closes [#561](https://github.com/hyperium/hyper/issues/561))
  * Add Access-Control-Allow-Credentials header ([19348b89](https://github.com/hyperium/hyper/commit/19348b892be4687e2c0e48b3d01562562340aa1f), closes [#655](https://github.com/hyperium/hyper/issues/655))
  * re-export CookiePair and CookieJar ([799698ca](https://github.com/hyperium/hyper/commit/799698ca87bc8f2f5446e9cb1301e5976657db6b))


#### Breaking Changes

* This adds 2 required methods to the `NetworkStream`
  trait, `set_read_timeout` and `set_write_timeout`. Any local
  implementations will need to add them.

 ([fec6e3e8](https://github.com/hyperium/hyper/commit/fec6e3e873eb79bd17d1c072d2ca3c7b91624f9c))
* LanguageTags api is changed.

 ([c747f99d](https://github.com/hyperium/hyper/commit/c747f99d2137e03b5f4393ee3731f6ebeab9ee6e))


### v0.6.16 (2015-11-16)


#### Bug Fixes

* **response:** respond with a 500 if a handler panics ([63c6762c](https://github.com/hyperium/hyper/commit/63c6762c15ec790f54391a71794315599ae0ced8))


#### Features

* **headers:** Add Access-Control-Expose-Headers ([f783e991](https://github.com/hyperium/hyper/commit/f783e9913b988f3d5c28707e2291145999756dbe))
* **server:** Add hooks for HttpListener and HttpsListener to be started from existing listener ([fa0848d4](https://github.com/hyperium/hyper/commit/fa0848d4216aa81e7b7619b7ce0a650356ee7ab7))


#### Breaking Changes

* `RequestBuilder<U>` should be replaced by `RequestBuilder`.

 ([ff4a6070](https://github.com/hyperium/hyper/commit/ff4a6070573955d1623d51a3d5302a17eed8f8d6))


### v0.6.15 (2015-10-09)


#### Bug Fixes

* **server:** use a timeout for Server keep-alive ([cdaa2547](https://github.com/hyperium/hyper/commit/cdaa2547ed18dfb0e3b8ed2ca15cfda1f98fa9fc), closes [#368](https://github.com/hyperium/hyper/issues/368))


#### Features

* **client:** add patch method to Client builder interface ([03827c31](https://github.com/hyperium/hyper/commit/03827c3156b5c0a7c865c5846aca2c1ce7a9f4ce))


### v0.6.14 (2015-09-21)


#### Bug Fixes

* **http:**
  * Add a stream enum that makes it impossible to lose a stream ([be4e7181](https://github.com/hyperium/hyper/commit/be4e7181456844180963d0e5234656c319ce92a6))
  * Make sure not to lose the stream when CL is invalid ([a36e44af](https://github.com/hyperium/hyper/commit/a36e44af7d4e665a122c1498011ff10035f7376f))
* **server:** use EmptyWriter for status codes that have no body ([9b2998bd](https://github.com/hyperium/hyper/commit/9b2998bddc3c033e4fc4e6a9b7d18504339ded3f))
* **timeouts:** remove rust #![feature] for socket timeouts ([b8729698](https://github.com/hyperium/hyper/commit/b872969880be502b681def26d6b9780cc90ac74b))


#### Features

* **headers:** add PartialEq impl for Headers struct ([76cbf384](https://github.com/hyperium/hyper/commit/76cbf384231e602d888e49932bf9c4fafdd88051))


### v0.6.13 (2015-09-02)


#### Bug Fixes

* **client:** EofReader by nature means the connection is closed ([32e09a04](https://github.com/hyperium/hyper/commit/32e09a04292b0247456a8fb9003a75a6abaa998e))


### v0.6.12 (2015-09-01)


#### Bug Fixes

* **client:** be resilient to invalid response bodies ([75c71170](https://github.com/hyperium/hyper/commit/75c71170206db3119d9b298ea5cf3ee860803124), closes [#640](https://github.com/hyperium/hyper/issues/640))
* **examples:** "cargo test --features serde-serialization" ([63608c49](https://github.com/hyperium/hyper/commit/63608c49c0168634238a119eb64ea1074df1b7e6))
* **http:** fix several cases in HttpReader ([5c7195ab](https://github.com/hyperium/hyper/commit/5c7195ab4a213bf0016f2185a63a6341e4cef4de))


#### Features

* **server:** Add Handler per-connection hooks ([6b6182e8](https://github.com/hyperium/hyper/commit/6b6182e8c4c81f634becebe7b45dc21bff59a286))


### v0.6.11 (2015-08-27)


#### Bug Fixes

* **client:** fix panics when some errors occurred inside HttpMessage ([ef15257b](https://github.com/hyperium/hyper/commit/ef15257b733d40bc3a7c598f61918f91385585f9))
* **headers:** case insensitive values for Connection header ([341f8eae](https://github.com/hyperium/hyper/commit/341f8eae6eb33e2242be09541807cdad9afc732e), closes [#635](https://github.com/hyperium/hyper/issues/635))


#### Breaking Changes

* This changes the signature of HttpWriter.end(),
  returning a `EndError` that is similar to std::io::IntoInnerError,
  allowing HttpMessage to retrieve the broken connections and not panic.

  The breaking change isn't exposed in any usage of the `Client` API,
  but for anyone using `HttpWriter` directly, since this was technically
  a public method, that change is breaking.

 ([ef15257b](https://github.com/hyperium/hyper/commit/ef15257b733d40bc3a7c598f61918f91385585f9))


### v0.6.10 (2015-08-19)


#### Bug Fixes

* **client:** close connection when there is an Error ([d32d35bb](https://github.com/hyperium/hyper/commit/d32d35bbea947172224082e1f9b711022ce75e30))


#### Features

* **uri:** implement fmt::Display for RequestUri () ([80931cf4](https://github.com/hyperium/hyper/commit/80931cf4c31d291125700ed3f9be5b3cb015d797), closes [#629](https://github.com/hyperium/hyper/issues/629))


### v0.6.9 (2015-08-13)


#### Bug Fixes

* **client:**
  * improve keep-alive of bodyless Responses ([67c284a9](https://github.com/hyperium/hyper/commit/67c284a96a006f888f43d8af929516465de76dea))
  * improve HttpReader selection for client Responses ([31f117ea](https://github.com/hyperium/hyper/commit/31f117ea08c01889016fd45e7084e9a049c53f7a), closes [#436](https://github.com/hyperium/hyper/issues/436))
* **nightly:** remove feature flag for duration ([0455663a](https://github.com/hyperium/hyper/commit/0455663a98d7969c23d64d0b775799342507ef8e))


#### Features

* **headers:** Content-Range header ([af062ac9](https://github.com/hyperium/hyper/commit/af062ac954d5b90275138880ce2f5013d6664b5a))
* **net:** impl downcast methods for NetworkStream (without + Send) ([1a91835a](https://github.com/hyperium/hyper/commit/1a91835abaa804aabf2e9bb45e9ab087274b8a18), closes [#521](https://github.com/hyperium/hyper/issues/521))
* **server:** add Request.ssl() to get underlying ssl stream ([7909829f](https://github.com/hyperium/hyper/commit/7909829f98bd9a2f454430f89b6143b977aedb35), closes [#627](https://github.com/hyperium/hyper/issues/627))


### v0.6.8 (2015-08-03)


#### Features

* **raw-fd:** implement FromRawFd/FromRawSocket ([664bde58](https://github.com/hyperium/hyper/commit/664bde58d8a6b2d6ce5624ed96b8d6d68214a782))


### v0.6.7 (2015-08-03)


#### Bug Fixes

* **headers:** fix broken deserialization of headers ([f5f5e1cb](https://github.com/hyperium/hyper/commit/f5f5e1cb2d01a22f170432e73b9c5757380cc18b))


#### Features

* **net:**
  * Implement NetworkConnector for closure to be more flexible ([abdd4c5d](https://github.com/hyperium/hyper/commit/abdd4c5d632059ebef9bbee95032c9500620212e))
  * add socket timeouts to Server and Client ([7d1f154c](https://github.com/hyperium/hyper/commit/7d1f154cb7b4db4a029b52857c377000a3f23419), closes [#315](https://github.com/hyperium/hyper/issues/315))


#### Breaking Changes

* Any custom implementation of NetworkStream must now
  implement `set_read_timeout` and `set_write_timeout`, so those will
  break. Most users who only use the provided streams should work with
  no changes needed.

Closes #315

 ([7d1f154c](https://github.com/hyperium/hyper/commit/7d1f154cb7b4db4a029b52857c377000a3f23419))


### v0.6.5 (2015-07-23)


#### Bug Fixes

* **tests:** iter.connect() is now iter.join() ([d2e8b5dc](https://github.com/hyperium/hyper/commit/d2e8b5dc3d2e6f0386656f4a5926acb848d4a61d))


#### Features

* **status:**
  * implement `Hash` for `StatusCode` ([d84f291a](https://github.com/hyperium/hyper/commit/d84f291abc0a64e270143eee943a76a7aebec029))
  * implement `Hash` for `StatusCode` ([aa85f609](https://github.com/hyperium/hyper/commit/aa85f609b5136cb2a9b23408a2b125c6a8a20f37))


### v0.6.4 (2015-07-23)


#### Features

* **http:** add optional serialization of common types via `serde` ([87de1b77](https://github.com/hyperium/hyper/commit/87de1b77bcd5533c70a6ab9379121001acc5d366))


### v0.6.3 (2015-07-08)


#### Bug Fixes

* **lint:** change deny(missing_docs) to only apply for tests ([5994a6f8](https://github.com/hyperium/hyper/commit/5994a6f8b4e48c9ab766e425dba210bdac59b00b), closes [#600](https://github.com/hyperium/hyper/issues/600))


### v0.6.2 (2015-07-06)


#### Bug Fixes

* **http:** no longer keep alive for Http1.0 if no Connection header ([ddecb262](https://github.com/hyperium/hyper/commit/ddecb262b39b90e594a95ba16c4dc8085930677e), closes [#596](https://github.com/hyperium/hyper/issues/596))


#### Features

* **client:** add url property Response ([82ed9092](https://github.com/hyperium/hyper/commit/82ed9092e30385de004912582a7838e037497c82))
* **headers:** add strict-transport-security header ([7c2e5124](https://github.com/hyperium/hyper/commit/7c2e5124e6678a5806f980902031e6f01652d218), closes [#589](https://github.com/hyperium/hyper/issues/589))


#### Breaking Changes

* Access-Control-Allow-Origin does no longer use Url

 ([ed458628](https://github.com/hyperium/hyper/commit/ed458628e54bd241b45f50fb0cf55a84ffb12205))
* Technically a break, since `Response::new()` takes an
  additional argument. In practice, the only place that should have been
  creating Responses directly is inside the Client, so it shouldn't
  break anyone. If you were creating Responses manually, you'll need to
  pass a Url argument.

 ([82ed9092](https://github.com/hyperium/hyper/commit/82ed9092e30385de004912582a7838e037497c82))


### v0.6.1 (2015-06-26)


#### Bug Fixes

* **benches:** adjust to missing `set_ssl_verifier` ([eb38a11b](https://github.com/hyperium/hyper/commit/eb38a11b9ab401d6b909077f92507fa872349d13))
* **cargo:** fix linking on OSX 10.10 ([9af2b66f](https://github.com/hyperium/hyper/commit/9af2b66fe4003706517d95ed94013af9cd365b24))
* **client:** use Ssl instance in creation of SslStream ([1a490e25](https://github.com/hyperium/hyper/commit/1a490e25c321bdd173d47ed7a7a704039746fb29))


## v0.6.0 (2015-06-24)


#### Bug Fixes

* **client:** check for drained stream in Response::drop ([e689f203](https://github.com/hyperium/hyper/commit/e689f20376d3e078f5d380902d39f8ae9c043486))


#### Features

* **client:**
  * impl Sync for Client ([64e47b4b](https://github.com/hyperium/hyper/commit/64e47b4bbd0433065a059804adeb2b4a2d72f327), closes [#254](https://github.com/hyperium/hyper/issues/254))
  * implement Protocol trait for HTTP/1.1 ([dccdf8d6](https://github.com/hyperium/hyper/commit/dccdf8d65a9b900daec34555d3b97c2c3c678067))
  * add `Protocol` trait ([3417303a](https://github.com/hyperium/hyper/commit/3417303a4a9aa4809729d53f0d018338e876da51))
  * implement HttpMessage for HTTP/1.1 ([ecb713f8](https://github.com/hyperium/hyper/commit/ecb713f8494b13bdba91258b1507e8f7ce62b8d9))
  * add `HttpMessage` trait ([289fd02b](https://github.com/hyperium/hyper/commit/289fd02b55a42748cbce8de428939208713a765d))
* **error:** add private `__Nonexhaustive` variant to Error ([7c0421e3](https://github.com/hyperium/hyper/commit/7c0421e3fc1d5a8b4868b57acca87abd685f3430))
* **headers:**
  * add bearer token support ([edf6ac20](https://github.com/hyperium/hyper/commit/edf6ac2074d11694ded275807a66df3a8a8e33a6))
  * add `Range` header ([05c31998](https://github.com/hyperium/hyper/commit/05c319984630b31d18dfbfa9b7567f6c7613d7f8))
* **http2:**
  * implement message API for HTTP/2 ([f0fe2c5a](https://github.com/hyperium/hyper/commit/f0fe2c5a83bd4e654a4ff684f75a1b602f8f38fc))
  * add new error variant for HTTP/2 ([48e9ca2f](https://github.com/hyperium/hyper/commit/48e9ca2f70f6c6475f1579ae9212af7b4ca87e88))
  * add dependency on `solicit` ([3122ffef](https://github.com/hyperium/hyper/commit/3122ffefc2d56ffc03a6fcc264086df0c9d74083))
* **langtags:** use true language tags in headers ([99ff7e62](https://github.com/hyperium/hyper/commit/99ff7e62573865a1fc431db26b6a18c43b9127de))
* **ssl:** redesign SSL usage ([53bba6eb](https://github.com/hyperium/hyper/commit/53bba6eb7f34e61e5c8a835281d625436532de8f))


#### Breaking Changes

* AcceptLanguage and ContentLanguage use LanguageTag now,
Language removed from Hyper.

 ([99ff7e62](https://github.com/hyperium/hyper/commit/99ff7e62573865a1fc431db26b6a18c43b9127de))
* Server::https was changed to allow any implementation
  of Ssl. Server in general was also changed. HttpConnector no longer
  uses SSL; using HttpsConnector instead.

 ([53bba6eb](https://github.com/hyperium/hyper/commit/53bba6eb7f34e61e5c8a835281d625436532de8f))
* Connectors and Protocols passed to the `Client` must
  now also have a `Sync` bounds, but this shouldn't break default usage.

 ([64e47b4b](https://github.com/hyperium/hyper/commit/64e47b4bbd0433065a059804adeb2b4a2d72f327))
* parse_header returns Result instead of Option, related
code did also change

 ([195a89fa](https://github.com/hyperium/hyper/commit/195a89fa918a83c9dcab47a4b09edb464d4e8006))
* Adds a new variant to public Error enum. The proper fix
  is to stop matching exhaustively on `hyper::Error`.

 ([7c0421e3](https://github.com/hyperium/hyper/commit/7c0421e3fc1d5a8b4868b57acca87abd685f3430))
* A new variant `Http2` added to a public enum
`hyper::Error`.

 ([48e9ca2f](https://github.com/hyperium/hyper/commit/48e9ca2f70f6c6475f1579ae9212af7b4ca87e88))
* `hyper::client::request::Response` is no longer generic
over `NetworkStream` types. It no longer requires a generic type
parameter at all.

 ([aa297f45](https://github.com/hyperium/hyper/commit/aa297f45322d66980bb2b51c413b15dfd51533ea))


### v0.5.2 (2015-06-01)


#### Bug Fixes

* **buffer:** check capacity before resizing ([b1686d1b](https://github.com/hyperium/hyper/commit/b1686d1b22aa95a17088f99054d577bbb2aef9dc))


### v0.5.1 (2015-05-25)


#### Bug Fixes

* **client:** don't close stream until EOF ([a5e6174e](https://github.com/hyperium/hyper/commit/a5e6174efd57afb1df7113c64f4e7718a3a94187), closes [#543](https://github.com/hyperium/hyper/issues/543))


#### Features

* **client:** implement Default trait for client ([be041d91](https://github.com/hyperium/hyper/commit/be041d915a55fa1b5088e112b81727b864949976))
* **header:** add ContentType::form_url_encoded() constructor ([2c99d4e9](https://github.com/hyperium/hyper/commit/2c99d4e9068b30ecb6d4eac4d364924fb253fdcd))
* **headers:** return hyper::Error instead of () from header components ([5d669399](https://github.com/hyperium/hyper/commit/5d669399b6ca5ec7d0f01b9d30513cd1cc4cc47b))
* **http:** add get_mut method to HttpReader ([e64ce8c0](https://github.com/hyperium/hyper/commit/e64ce8c05e847b2396e4b7e2bb656240e9806ed8))


#### Breaking Changes

* Error enum extended. Return type of header/shared/
types changed.

 ([5d669399](https://github.com/hyperium/hyper/commit/5d669399b6ca5ec7d0f01b9d30513cd1cc4cc47b))


## v0.5.0 (2015-05-12)


#### Bug Fixes

* **client:**
  * don't call close() inside Request ([3334fca2](https://github.com/hyperium/hyper/commit/3334fca278e662b2755e41045ce641238514bea9), closes [#519](https://github.com/hyperium/hyper/issues/519))
  * keep the underlying connector when setting an SSL verifier ([f4556d55](https://github.com/hyperium/hyper/commit/f4556d554faa2a1170fec0af5b4076c31e7c3600), closes [#495](https://github.com/hyperium/hyper/issues/495))
* **mock:** adjust ChannelMockConnector connect method to compile ([085d7b07](https://github.com/hyperium/hyper/commit/085d7b0752d7fc0134e99e9eec2a67cc66b319b3))


#### Features

* **header:**
  * add ContentType::json(), plaintext(), html(), jpeg(), and png() constructors ([b6114ecd](https://github.com/hyperium/hyper/commit/b6114ecd2e65bd59e79a67a45913adaf0f1552f0))
  * add Connection::close() and ::keep_alive() constructors ([c2938fb4](https://github.com/hyperium/hyper/commit/c2938fb45f9c1fff2a1235d82b7741531de21445))
  * export __hyper__tm! macro so test modules work with header! ([f64fb10b](https://github.com/hyperium/hyper/commit/f64fb10bc87bb4b5a5291d09364ad6c725a842d8))
* **net:**
  * remove mut requirement for NetworkConnector.connect() ([1b318724](https://github.com/hyperium/hyper/commit/1b318724a5fd425366daddf15c5964d7c3cbc240))
  * add `set_ssl_verifier` method to `NetworkConnector` trait ([a5d632b6](https://github.com/hyperium/hyper/commit/a5d632b6ea53d0988d6383dd734d0b5e6245ba2b))
* **server:** check Response headers for Connection: close in keep_alive loop ([49b5b8fd](https://github.com/hyperium/hyper/commit/49b5b8fdfe256ead8f3aa3d489bc4b299c190a9a))


#### Breaking Changes

* Usage of Response.deconstruct() and construct() now use
  a &mut Headers, instead of the struct proper.

 ([49b5b8fd](https://github.com/hyperium/hyper/commit/49b5b8fdfe256ead8f3aa3d489bc4b299c190a9a))
* If you use deref! from the header module, you'll need
  to switch to using __hyper__deref!.

 ([62d96adc](https://github.com/hyperium/hyper/commit/62d96adc6b852b3836b47fc2e154bbdbab9ad7f6))
* Any custom Connectors will need to change to &self in
  the connect method. Any Connectors that needed the mutability need to
  figure out a synchronization strategy.

  Request::with_connector() takes a &NetworkConnector instead of &mut.
  Any uses of with_connector will need to change to passing &C.

 ([1b318724](https://github.com/hyperium/hyper/commit/1b318724a5fd425366daddf15c5964d7c3cbc240))
* Adding a new required method to a public trait is a
breaking change.

 ([a5d632b6](https://github.com/hyperium/hyper/commit/a5d632b6ea53d0988d6383dd734d0b5e6245ba2b))


## v0.4.0 (2015-05-07)


#### Bug Fixes

* **net:** ignore NotConnected error in NetworkStream.close ([6be60052](https://github.com/hyperium/hyper/commit/6be60052c627b7e498d973465b4a3ee7efc40665), closes [#508](https://github.com/hyperium/hyper/issues/508))


#### Features

* **error:** add Ssl variant to hyper::Error ([972b3a38](https://github.com/hyperium/hyper/commit/972b3a388ac3af98ba038927c551b92be3a68d62), closes [#483](https://github.com/hyperium/hyper/issues/483))
* **headers:**
  * Allow `null` value in Access-Control-Allow-Origin ([5e341714](https://github.com/hyperium/hyper/commit/5e3417145ced116147ef1e890b4f1e7c775ad173))
  * Parse Upgrade header protocols further ([f47d11b9](https://github.com/hyperium/hyper/commit/f47d11b97bb4a4bf67c3f9aa47c203babf4a9c72), closes [#480](https://github.com/hyperium/hyper/issues/480))
  * Add From header field ([ce9c4af1](https://github.com/hyperium/hyper/commit/ce9c4af1e0a46abc9f7908c2cb0659a2ecab137c))
  * Add Accept-Ranges header field ([2dbe3f9b](https://github.com/hyperium/hyper/commit/2dbe3f9b9a3fc9f04346712e55f40dabaf72d9a8))
* **method:** implement `AsRef<str>` for `Method` ([c29af729](https://github.com/hyperium/hyper/commit/c29af729726ae782bece5e790bce02b0d3ab9ef9))
* **server:**
  * add Response.send to write a sized body ([d5558b68](https://github.com/hyperium/hyper/commit/d5558b687d32d0affb9aaa7185227a4e294f5454), closes [#446](https://github.com/hyperium/hyper/issues/446))
  * dropping a Response will write out to the underlying stream ([a9dcc59c](https://github.com/hyperium/hyper/commit/a9dcc59cd9846609a5733678f66353655c075279))


#### Breaking Changes

* Adds a variant to `hyper::Error`, which may break any
exhaustive matches.

 ([972b3a38](https://github.com/hyperium/hyper/commit/972b3a388ac3af98ba038927c551b92be3a68d62))
* The terms `Http` and `Error` have been removed from the Error
  type and its variants. `HttpError` should now be accessed as `hyper::Error`,
  and variants like `HttpIoError` should be accessed as `Error::Io`.

 ([9ba074d1](https://github.com/hyperium/hyper/commit/9ba074d150a55a749161317405fe8b28253c5a9d))
* Add variant to Access-Control-Allow-Origin enum

 ([5e341714](https://github.com/hyperium/hyper/commit/5e3417145ced116147ef1e890b4f1e7c775ad173))
* Upgrade header Protocol changed.

 ([f47d11b9](https://github.com/hyperium/hyper/commit/f47d11b97bb4a4bf67c3f9aa47c203babf4a9c72))
* `from_one_raw_str()` returns `None` on empty values.

 ([a6974c99](https://github.com/hyperium/hyper/commit/a6974c99d39fcbaf3fb9ed38428b21e0301f3602))


### v0.3.16 (2015-05-01)


#### Bug Fixes

* **header:**
  * make test_module of header! optional ([a5ce9c59](https://github.com/hyperium/hyper/commit/a5ce9c59fa61410551b07252364564a2bb13bb86), closes [#490](https://github.com/hyperium/hyper/issues/490))
  * exporting test_header! macro ([2bc5a779](https://github.com/hyperium/hyper/commit/2bc5a779bdc3fce67e06c398ac8702fcbea93dab))
* **http:** keep raw reason phrase in RawStatus ([8cdb9d5d](https://github.com/hyperium/hyper/commit/8cdb9d5d3b0972629e8843d3c1db58dbbbaf49cf), closes [#497](https://github.com/hyperium/hyper/issues/497))


#### Features

* **client:** add a Connection Pool ([1e72a8ab](https://github.com/hyperium/hyper/commit/1e72a8ab3a0092bb863686ad2e65646710706c1b), closes [#363](https://github.com/hyperium/hyper/issues/363), [#41](https://github.com/hyperium/hyper/issues/41))
* **headers:** Add If-Range header ([a39735f1](https://github.com/hyperium/hyper/commit/a39735f1d3d1a314969b5b0085e8f77f0c10c863), closes [#388](https://github.com/hyperium/hyper/issues/388))


### v0.3.15 (2015-04-29)


#### Bug Fixes

* **headers:**
  * Do not parse empty values in list headers. ([093a29ba](https://github.com/hyperium/hyper/commit/093a29bab7eb27e78bb10506478ac486e8d61671))
  * Fix formatting of 0 qualites and formatting of empty list header fields. ([621ef521](https://github.com/hyperium/hyper/commit/621ef521f6723ba2d59beff05ff39ae8fd6df2c3))


#### Features

* **client:**
  * remove Clone requirement for NetworkStream in Client ([60d92c29](https://github.com/hyperium/hyper/commit/60d92c296a445b352679919c03c5ed2a2a297e16))
  * accept &String as Body in RequestBuilder ([a2aefd9a](https://github.com/hyperium/hyper/commit/a2aefd9a5689d4816f7c054bd6c32aa5c6fe3087))
  * accept &String for a Url in RequestBuilder ([8bc179fb](https://github.com/hyperium/hyper/commit/8bc179fb517735a7c1d5cd1d7f5598bb82914dc6))
* **headers:** Implement Content-Language header field ([308880b4](https://github.com/hyperium/hyper/commit/308880b455df4dbb5d32817b5c0320c2a88139e3), closes [#475](https://github.com/hyperium/hyper/issues/475))
* **net:** add https_using_context for user-supplied SslContext ([1a076d1b](https://github.com/hyperium/hyper/commit/1a076d1bc7e8fb9c58904b0cec879dcf0fbce97b))
* **server:** allow consumer to supply an SslContext ([3a1a2427](https://github.com/hyperium/hyper/commit/3a1a24270dd13e22ef59120d66d327528949d5e0), closes [#471](https://github.com/hyperium/hyper/issues/471))


#### Breaking Changes

* This removes the trait `IntoBody`, and instead using
  `Into<Body>`, as it's more idiomatic. This will only have broken code
  that had custom implementations of `IntoBody`, and can be fixed by
  changing them to `Into<Body>`.

 ([a2aefd9a](https://github.com/hyperium/hyper/commit/a2aefd9a5689d4816f7c054bd6c32aa5c6fe3087))


### v0.3.14 (2015-04-18)


#### Bug Fixes

* **http:** Adjust httparse Request and Response lifetimes. ([76550fdb](https://github.com/hyperium/hyper/commit/76550fdb20bb812e92a1fc3f3a7eaaf4a689348b))


### v0.3.13 (2015-04-17)


#### Bug Fixes

* **server:** JoinHandle type parameter ([c694b138](https://github.com/hyperium/hyper/commit/c694b1385bd294e7c8e0398ee75e3a054ced5006))


#### Features

* **debug:** add Debug impls for StatusClass, Server, and Listening ([0fb92ee7](https://github.com/hyperium/hyper/commit/0fb92ee735136a07c832124df521b96a6779bd39))


### v0.3.12 (2015-04-15)


#### Bug Fixes

* **server:**
  * handle keep-alive closing ([d9187713](https://github.com/hyperium/hyper/commit/d9187713b2eaa628eb34f68c8a7201a6cf8e010d), closes [#437](https://github.com/hyperium/hyper/issues/437))
  * join on thread when Listening drops ([68d4d63c](https://github.com/hyperium/hyper/commit/68d4d63c2a0289b72ec1442d13e1212a0479c50b), closes [#447](https://github.com/hyperium/hyper/issues/447))
  * Use thread::spawn instead of thread::scoped. ([e8649567](https://github.com/hyperium/hyper/commit/e864956734af72bab07a3e01c9665bc1b7c96e5e))


#### Features

* **http:** Implement Debug for HttpReader/Writer. ([2f606c88](https://github.com/hyperium/hyper/commit/2f606c88bd91e5e36dee4c6db00c3117b1adf067))
* **log:** clean up logging ([4f09b002](https://github.com/hyperium/hyper/commit/4f09b002ffb2d076fc8fb01d9b9e0464216b2b41))
* **net:** make HttpStream implement Debug ([7b7f9c25](https://github.com/hyperium/hyper/commit/7b7f9c257d0e2d515bf336c567f12a625471e477))


### v0.3.11 (2015-04-15)


#### Bug Fixes

* **headers:** Content-Encoding needs a hyphen. ([ca2815ef](https://github.com/hyperium/hyper/commit/ca2815effda2a5b27f781b7bc35105aa81121bae))


#### Features

* **client:** remove generic parameter for Connector ([139a51f1](https://github.com/hyperium/hyper/commit/139a51f1c31b80cdddf643e984bbbfbb3d3e8c96), closes [#379](https://github.com/hyperium/hyper/issues/379))


#### Breaking Changes

* `AccessControlAllowHeaders` and `AccessControlRequestHeaders` values
are case insensitive now. `AccessControlAllowOrigin` variants are now `Any` and
`Value` to match the other headers.

 ([94f38950](https://github.com/hyperium/hyper/commit/94f38950ddf9a97fdc4f44e42aada4ed8f4d9b43))
* `If-Match`, `If-None-Match` and `Vary` item variant name changed to `Items`

 ([38d297b1](https://github.com/hyperium/hyper/commit/38d297b16e5d14d533947988f770f03b49d47a17))
* `Etag` header field is now `ETag` header field

 ([4434ea6a](https://github.com/hyperium/hyper/commit/4434ea6a7d57d367c0a541c82f6289ffbda5fb6c))
* For people using the default HttpConnector and Client,
    everything should continue to just work. If the Client has been
    used with a generic parameter, it should be removed.

    However, there were some breaking changes to the internals of
    NetworkConnectors. Specifically, they no longer return a
    NetworkStream, but instead a Into<Box<NetworkStream + Send>>. All
    implementations of NetworkStream should continue to just work,
    however.

    Possible breakages could come from the stricter usage of Send
    throughout the Client API.

 ([139a51f1](https://github.com/hyperium/hyper/commit/139a51f1c31b80cdddf643e984bbbfbb3d3e8c96))


### v0.3.10 (2015-04-06)


#### Bug Fixes

* **README:** Update to compile example against Rust beta ([341f19d3](https://github.com/hyperium/hyper/commit/341f19d3266c6de9a9a90c94f718124792766630))


### v0.3.9 (2015-04-03)


#### Bug Fixes

* **headers:** Add CowStr as a temporary hack to build on beta. ([8e065563](https://github.com/hyperium/hyper/commit/8e0655637e80c5377c01da4dbca6fb627e6d4225))


### v0.3.8 (2015-04-02)


#### Bug Fixes

* **rustup:** update to rust beta ([0f5858f3](https://github.com/hyperium/hyper/commit/0f5858f37974731243d47710364776fdd73376fe))


#### Breaking Changes

* Removed impl_header!() and impl_list_header!() macros,
use new header!() macro.

 ([262c450f](https://github.com/hyperium/hyper/commit/262c450f908dbf27754daff0784f0f20145036dd))


### v0.3.7 (2015-03-31)


#### Bug Fixes

* **buffer:** zero out new capacity when buffer grows ([cfdabd70](https://github.com/hyperium/hyper/commit/cfdabd70ecc3f5290ae1e6f7e5dfd50310d8658d))


#### Features

* **entitytag:** Add EntityTag comparison, make EntityTag safe to use ([9c21f7f9](https://github.com/hyperium/hyper/commit/9c21f7f953a5163792e71fb186cab391c45d1bb4))


### v0.3.6 (2015-03-30)


#### Bug Fixes

* **buffer:** get_buf to not return consumed part of buffer ([04e3b565](https://github.com/hyperium/hyper/commit/04e3b5651561f087fee7c0345fe77d217d3ad35a), closes [#406](https://github.com/hyperium/hyper/issues/406))
* **rustup:** get rid of slice pattern, add `Reflect` bounds ([c9f2c841](https://github.com/hyperium/hyper/commit/c9f2c841ff0e68dead38e762ed5f8c0f42255bc4))


### v0.3.5 (2015-03-28)


#### Bug Fixes

* **http:** read more before triggering TooLargeError ([cb59f609](https://github.com/hyperium/hyper/commit/cb59f609c61a097d5d9fa728b9df33d79922573b), closes [#389](https://github.com/hyperium/hyper/issues/389))


### v0.3.4 (2015-03-26)


#### Bug Fixes

* **rustup:** static bounds required on Type definition, trivial_casts ([eee7a85d](https://github.com/hyperium/hyper/commit/eee7a85d3c3a3f51a1c3c12496c0e45ea312524e))


### v0.3.3 (2015-03-25)


#### Bug Fixes

* **rustup:**
  * rustc 1.0.0-nightly (123a754cb 2015-03-24) ([3e456f00](https://github.com/hyperium/hyper/commit/3e456f00f9991b1c723a232fc9c76fe8c0539858))
  * 1.0.0-nightly (e2fa53e59 2015-03-20) ([f547080d](https://github.com/hyperium/hyper/commit/f547080df53076711b52a016b990c5be56f42ede))


#### Features

* **headers:** Implementing content-encoding header ([2983e8de](https://github.com/hyperium/hyper/commit/2983e8dea21f02a31012a25b0a302a128768030a), closes [#391](https://github.com/hyperium/hyper/issues/391))


### v0.3.2 (2015-03-20)


#### Bug Fixes

* **benches:** removed unused features ([104d4903](https://github.com/hyperium/hyper/commit/104d49036ff40c730ec8bef8012f19ccbee4aaae))
* **rustup:**
  * rustc 1.0.0-nightly (ea8b82e90) ([8181de25](https://github.com/hyperium/hyper/commit/8181de253aecfe81123e166a141ebfc8430ec4a4))
  * adapt to current rustc ([1f0bc951](https://github.com/hyperium/hyper/commit/1f0bc951c9ee40cab622a72d614d4c45d889ccd3), closes [#381](https://github.com/hyperium/hyper/issues/381))


#### Features

* **server:** use SocketAddrs instead of Ipv4Addrs ([5d7be77e](https://github.com/hyperium/hyper/commit/5d7be77e4ac0d5c1d852c1208abc77a913c4f4d1))


### v0.3.1 (2015-03-18)


#### Bug Fixes

* **header:** Fix charset parsing bug. ([5a6e176f](https://github.com/hyperium/hyper/commit/5a6e176f50fe667fbdc4c933c81d2db5ba5c571d))
* **headers:** Fix overflow with empty cookies ([99baaa10](https://github.com/hyperium/hyper/commit/99baaa10157f6c69ef1795a97e0db8bd794011f6))
* **rustup:** update to latest rustc ([4fd8a6a9](https://github.com/hyperium/hyper/commit/4fd8a6a9dc0dc969b36f3d3ad51cee177545f883))


#### Features

* **server:** add Expect 100-continue support ([0b716943](https://github.com/hyperium/hyper/commit/0b7169432b5f51efe5c167be418c2c50220e46a5), closes [#369](https://github.com/hyperium/hyper/issues/369))


#### Breaking Changes

* Several public functions and types in the `http` module
  have been removed. They have been replaced with 2 methods that handle
  all of the http1 parsing.

 ([b87bb20f](https://github.com/hyperium/hyper/commit/b87bb20f0c25891c30ef2399da2721596fbc1fcf))


## v0.3.0 (2015-03-03)


#### Features

* **headers:**
  * add enum for Charset ([180d9a92](https://github.com/hyperium/hyper/commit/180d9a92d92541aa415c918a2265bd6b33d39655))
  * add AcceptCharset header ([235089a1](https://github.com/hyperium/hyper/commit/235089a1034dc93ca62f47dcab0a93f1d49c72dd))
  * add q function to ease creating Quality values ([d68773c7](https://github.com/hyperium/hyper/commit/d68773c79f998813bbd1bf50a0dbc2bc01ee0470))
  * adds re-parsing ability when getting typed headers ([df756871](https://github.com/hyperium/hyper/commit/df756871edf4143135644c211106c5a8f8f5adb0))
* **hyper:** switch to std::io, std::net, and std::path. ([0fd6fcd7](https://github.com/hyperium/hyper/commit/0fd6fcd7c7f30c4317678a3b0968cc08ae9c0a71), closes [#347](https://github.com/hyperium/hyper/issues/347))


#### Breaking Changes

* added requirement that all HeaderFormat implementations
  must also be fmt::Debug. This likely as easy as slapping
  #[derive(Debug)] on to any custom headers.

 ([df756871](https://github.com/hyperium/hyper/commit/df756871edf4143135644c211106c5a8f8f5adb0))
* Check the docs. Everything was touched.

 ([0fd6fcd7](https://github.com/hyperium/hyper/commit/0fd6fcd7c7f30c4317678a3b0968cc08ae9c0a71))


### v0.2.1 (2015-02-27)


#### Bug Fixes

* **rustup:** str.split and associated type changes ([1b6e6a04](https://github.com/hyperium/hyper/commit/1b6e6a040fa26a8b3855ac46ccbcd5ee78065c71))


#### Features

* **headers:** add remove_raw method and corresponding test ([4f576780](https://github.com/hyperium/hyper/commit/4f576780c24ff3f943d5f821730ba65f4cdf8d4a), closes [#326](https://github.com/hyperium/hyper/issues/326))


## v0.2.0 (2015-02-21)


#### Bug Fixes

* **headers:** use $crate when referring to hyper modules on macros ([e246c3ac](https://github.com/hyperium/hyper/commit/e246c3ace8395cb5d281b841a416c503db1054ee), closes [#323](https://github.com/hyperium/hyper/issues/323))
* **rustup:**
  * Send changes ([4f5b97fe](https://github.com/hyperium/hyper/commit/4f5b97fefcea650214ca26c1aa197cd73683742f))
  * CowString is gone ([98b8c4b1](https://github.com/hyperium/hyper/commit/98b8c4b13723d8fa1b4f1ba42a06bb533bf13694))
  * Extend now takes an IntoIterator ([598d8f93](https://github.com/hyperium/hyper/commit/598d8f93e4a79dcc5ff58fbdc27e6b1a859786d1))
  * Add PhantomData markers to phantom type users ([1904c456](https://github.com/hyperium/hyper/commit/1904c4561f00a345714beadfa077016306b2c05d))
  * Remove uses of the obsolete &a[] syntax ([039e984f](https://github.com/hyperium/hyper/commit/039e984f6878d724d47f7e9fe7db765495ae2f10))
  * Fix signature of IntoCow ([234fcdc3](https://github.com/hyperium/hyper/commit/234fcdc3a25deb06240848d601be9e68930a73e6))
  * update feature flags ([b47f9365](https://github.com/hyperium/hyper/commit/b47f936525dde91b3456078ecf8d0c11917cc6b7))
  * use module-level thread functions ([fc2076cd](https://github.com/hyperium/hyper/commit/fc2076cd53c37ea244a0b89d7dd4b1eb8aeeb1d3))
  * update lifetime bounds ([f4a66b38](https://github.com/hyperium/hyper/commit/f4a66b38cb9e35bfec0bbc3c97e5298fc8ad8409))


#### Features

* **server:** make AcceptorPool::accept() block and allow non'-static data ([b0a72d80](https://github.com/hyperium/hyper/commit/b0a72d80d0e894220da6aa5ea29d71b278df596d))


### v0.1.13 (2015-02-17)


#### Bug Fixes

* **server:** Drain requests on drop. ([3d0f423e](https://github.com/hyperium/hyper/commit/3d0f423eb26c4f14aaf9f8a909b307f661a3c5d6), closes [#197](https://github.com/hyperium/hyper/issues/197), [#309](https://github.com/hyperium/hyper/issues/309))


#### Features

* **header:** Support arbitrary status codes ([73978531](https://github.com/hyperium/hyper/commit/7397853148b8221c0eb8315ae2e5f195ad2e642c))
* **headers:**
  * Implement PartialOrd for QualityItem ([2859d7ef](https://github.com/hyperium/hyper/commit/2859d7ef4ecadc3927fa46292ebbb225da597690), closes [#314](https://github.com/hyperium/hyper/issues/314))
  * add AcceptLanguage header ([20a585e3](https://github.com/hyperium/hyper/commit/20a585e30bbb060a91839de7e95fd75a95d03d93))
  * add IfMatch header ([5df06d44](https://github.com/hyperium/hyper/commit/5df06d4465fae01ef08b926f1f3be9f32a0f5c80))
* **server:** Rewrite the accept loop into a custom thread pool. ([3528fb9b](https://github.com/hyperium/hyper/commit/3528fb9b015a0959268452d5b42d5544c7b98a6a))


#### Breaking Changes

* This removes unregistered status codes from the enum. Use
`FromPrimitive` methods to create them now. StatusCode and StatusClass can no
longer be casted to `u16`, use `ToPrimitive` methods now.
For example `status.to_u16().unwrap()` to get the status code number.

 ([73978531](https://github.com/hyperium/hyper/commit/7397853148b8221c0eb8315ae2e5f195ad2e642c))


### v0.1.12 (2015-02-13)


#### Bug Fixes

* **net:** don't stop the server when an SSL handshake fails with EOF ([55f12660](https://github.com/hyperium/hyper/commit/55f12660891812d13a59e799b0ab5b185926479a))


#### Features

* **headers:** Add `If-None-Match` header field ([318b067a](https://github.com/hyperium/hyper/commit/318b067a06ecb42f0fba51928675d3b4291c7643), closes [#238](https://github.com/hyperium/hyper/issues/238))


### v0.1.11 (2015-02-06)


#### Bug Fixes

* **readme:** Make the README client example work ([9b5d6aab](https://github.com/hyperium/hyper/commit/9b5d6aab7e68cf776618151e9e69e34fd66aba6c))


#### Features

* **headers:** add IfUnmodifiedSince header ([b5543b67](https://github.com/hyperium/hyper/commit/b5543b67525e3d6ebc655d7e1736c8ade5b6dbb0))


#### Breaking Changes

* for any consumers of the Etag header, since the entity
tag is now in a tuple.

 ([28fd5c81](https://github.com/hyperium/hyper/commit/28fd5c81f54bb0ea3eda43a4014c736d00b4b07d))


### v0.1.10 (2015-02-03)


#### Bug Fixes

* **headers:** add limit to maximum header size that should be parsed ([f18a8fb7](https://github.com/hyperium/hyper/commit/f18a8fb76f15f36dec329683abb66be203ab2e7e), closes [#256](https://github.com/hyperium/hyper/issues/256))
* **rustup:**
  * update FromStr ([742081c8](https://github.com/hyperium/hyper/commit/742081c8cfeeb59908a653316a6377d05ffaa55c))
  * fix unused_feature warning in example server ([05a3a6b7](https://github.com/hyperium/hyper/commit/05a3a6b70badc28da33ff65e8c15003f87738e07))
  * switch to unstable features ([3af8b687](https://github.com/hyperium/hyper/commit/3af8b687d4a6ef462eb74b1f5a1cbb8f191902fd))


### v0.1.9 (2015-01-28)


#### Bug Fixes

* **headers:** Don't display q if q=1 in quality item. ([91df2441](https://github.com/hyperium/hyper/commit/91df2441a0bb8c032b6fc5ccff50ed0eb98f2194), closes [#281](https://github.com/hyperium/hyper/issues/281))
* **rustup:** update io import, Writer::write ([f606b603](https://github.com/hyperium/hyper/commit/f606b6039d15a0b6e46f5154a9c5482866497a0c))


#### Features

* **status:** add is_<status_class>() methods to StatusCodes ([2d55a22e](https://github.com/hyperium/hyper/commit/2d55a22e738fb7f37a271be4fc3cf2ebdb9b5345))


### v0.1.8 (2015-01-27)


#### Bug Fixes

* **headers:**
  * make ConnectionHeader unicase ([e06e7d9a](https://github.com/hyperium/hyper/commit/e06e7d9a7ece9588b673b06df6aec4663595df30))
  * make Protocol search websocket unicase ([65c70180](https://github.com/hyperium/hyper/commit/65c7018046eb556085ca47a28c980ec901980643))
* **log:** update to new logging levels ([b002b6c3](https://github.com/hyperium/hyper/commit/b002b6c3f09775e5d6759bbd07dacdee318c2915))


#### Features

* **headers:** Add `Pragma` header field ([767c95d2](https://github.com/hyperium/hyper/commit/767c95d2b9709b496b35d0d691ff7a1f6d35cbed), closes [#237](https://github.com/hyperium/hyper/issues/237))


#### Breaking Changes

* Change header `Cookie` to `Cookie`

 ([92f43cf8](https://github.com/hyperium/hyper/commit/92f43cf873ddceca9518195af6dad1ff6ac79e11))


### v0.1.7 (2015-01-27)


#### Bug Fixes

* **rustup:** update to newest fmt trait names and slice syntax ([9e3c94d7](https://github.com/hyperium/hyper/commit/9e3c94d764522f900731fdbdee857639901037fe))


#### Breaking Changes

* Implementations of Header will need to adjust the
    header_name method. It no longer takes any arguments.

 ([8215889e](https://github.com/hyperium/hyper/commit/8215889eda537d09da82a7ed12a1766bf4fd3bfe))


### v0.1.6 (2015-01-27)


#### Bug Fixes

* **headers:** make Schemes, Basic, Protocol public ([e43c35c1](https://github.com/hyperium/hyper/commit/e43c35c1ca86c0ff1278ccfe3d2cff43222627b2))


### v0.1.5 (2015-01-27)


### v0.1.4 (2015-01-27)


#### Bug Fixes

* **imports:** Update TypeID import location to "any" ([dd2534a6](https://github.com/hyperium/hyper/commit/dd2534a6863f8b3940d2776e6b6a8e48988b9b88))


### v0.1.3 (2015-01-27)


#### Features

* **server:** add a deconstruct method to Request. ([1014855f](https://github.com/hyperium/hyper/commit/1014855faec62ba00acdff6263c86e7dfa5fb047))


### v0.1.2 (2015-01-27)


#### Bug Fixes

* **server:** Increase MAX_HEADER_FIELD_LENGTH to 4k ([54238b28](https://github.com/hyperium/hyper/commit/54238b28e4899e76bb3d7c2dfd8d9bc6fd489b6c))


#### Features

* **net:**
  * Move SSL verification to unboxed closures ([bca9a53c](https://github.com/hyperium/hyper/commit/bca9a53c66c967affb8e245f26507494db39c35e))
  * Allow more generic SSL verification () ([af577851](https://github.com/hyperium/hyper/commit/af5778510d1d8422fcb04873f7c726a67f15f5eb), closes [#244](https://github.com/hyperium/hyper/issues/244))


### 0.1.1 (2015-01-13)

#### Features

* **server:**: Add TLS/SSL support serverside ([c6eef681](c6eef6812458e10de582530d7f2c5bce5156b73c), closes [#1](https://github.com/hyperium/hyper/issues/1))


#### Bug Fixes

* **headers:**
    * fix fmt_header outputs of several headers ([aa266653](https://github.com/hyperium/hyper/commit/aa26665367bde895ce02ad2a8e1a372f00719852), closes [#246](https://github.com/hyperium/hyper/issues/246))
    * don't use Show to write UserAgent header ([c8e334aa](https://github.com/hyperium/hyper/commit/c8e334aaebb5522a86d47f7e3c33836d2061cb65))
