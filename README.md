# [hyper](https://hyper.rs)

[![crates.io](https://meritbadge.herokuapp.com/hyper)](https://crates.io/crates/hyper)
[![Released API docs](https://docs.rs/hyper/badge.svg)](https://docs.rs/hyper)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![CI](https://github.com/hyperium/hyper/workflows/CI/badge.svg)](https://github.com/hyperium/hyper/actions?query=workflow%3ACI)

A **fast** and **correct** HTTP implementation for Rust.

**Get started** by looking over the [guides](https://hyper.rs/guides).

**Hyper is a relatively low-level library, if you are looking for simple
high-level HTTP client, then you may wish to consider
[reqwest](https://github.com/seanmonstar/reqwest), which is built on top of
this library.**

*NOTE*: hyper's [master](https://github.com/hyperium/hyper) branch is currently
preparing breaking changes, for most recently released code, look to the
[0.12.x](https://github.com/hyperium/hyper/tree/0.12.x) branch.

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
