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

