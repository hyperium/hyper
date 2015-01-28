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

