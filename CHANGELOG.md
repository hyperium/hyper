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

* **header:** Support arbitary status codes ([73978531](https://github.com/hyperium/hyper/commit/7397853148b8221c0eb8315ae2e5f195ad2e642c))
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

