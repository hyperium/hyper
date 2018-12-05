# [hyper](https://hyper.rs)

[![Travis Build Status](https://travis-ci.org/hyperium/hyper.svg?branch=master)](https://travis-ci.org/hyperium/hyper)
[![Coverage Status](https://coveralls.io/repos/hyperium/hyper/badge.svg?branch=master)](https://coveralls.io/r/hyperium/hyper?branch=master)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![crates.io](https://meritbadge.herokuapp.com/hyper)](https://crates.io/crates/hyper)
[![Released API docs](https://docs.rs/hyper/badge.svg)](https://docs.rs/hyper)

A **fast** and **correct** HTTP implementation for Rust.

**Get started** by looking over the [guides](https://hyper.rs/guides).

## Overview

hyper is a fast, safe HTTP implementation written in and for Rust.

hyper offers both an HTTP client and server which can be used to drive
complex web applications written entirely in Rust.

hyper makes use of "async IO" (non-blocking sockets) via the
[Tokio](https://tokio.rs) and [Futures](https://docs.rs/futures) crates.

Be aware that hyper is still actively evolving towards 1.0, and is likely
to experience breaking changes before stabilising. However, this mostly now
around the instability of `Future` and `async`. The rest of the API is rather
stable now. You can also see the
[issues in the upcoming milestones](https://github.com/hyperium/hyper/milestones).

## Contributing

To get involved, take a look at [CONTRIBUTING](CONTRIBUTING.md).

There are two main avenues for real-time chatting about hyper: a [Gitter room][gitter]
and [irc.mozilla.org/hyper][irc]. They are mirrored, so choose whichever format you
prefer.

[gitter]: https://gitter.im/hyperium/hyper
[irc]: https://kiwiirc.com/nextclient/irc.mozilla.org/#hyper

## License

hyper is provided under the MIT license. See [LICENSE](LICENSE).
