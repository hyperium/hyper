# [hyper](https://hyper.rs)

[![Travis Build Status](https://travis-ci.org/hyperium/hyper.svg?branch=master)](https://travis-ci.org/hyperium/hyper)
[![Appveyor Build status](https://ci.appveyor.com/api/projects/status/tb0n55fjs5tohdfo/branch/master?svg=true)](https://ci.appveyor.com/project/seanmonstar/hyper)
[![Coverage Status](https://coveralls.io/repos/hyperium/hyper/badge.svg?branch=master)](https://coveralls.io/r/hyperium/hyper?branch=master)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![crates.io](http://meritbadge.herokuapp.com/hyper)](https://crates.io/crates/hyper)
[![Released API docs](https://docs.rs/hyper/badge.svg)](http://docs.rs/hyper)
[![Master API docs](https://img.shields.io/badge/docs-master-green.svg)](http://hyperium.github.io/hyper/master)

A low-level HTTP implementation for Rust.

**Get started** by looking over the [guides](https://hyper.rs/guides).

## Overview

hyper is a fast, safe HTTP implementation written in and for Rust.

hyper offers both an HTTP client and server which can be used to drive
complex web applications written entirely in Rust.

hyper makes use of "async IO" (non-blocking sockets) via the
[Tokio](https://tokio.rs) and [Futures](https://docs.rs/futures) crates.

Be aware that hyper is still actively evolving towards 1.0, and is likely
to experience breaking changes before stabilising. You can also see the
[1.0 issue milestone](https://github.com/hyperium/hyper/milestone/1).
