# hyper

[![Travis Build Status](https://travis-ci.org/hyperium/hyper.svg?branch=master)](https://travis-ci.org/hyperium/hyper)
[![Appveyor Build status](https://ci.appveyor.com/api/projects/status/tb0n55fjs5tohdfo/branch/master?svg=true)](https://ci.appveyor.com/project/seanmonstar/hyper)
[![Coverage Status](https://coveralls.io/repos/hyperium/hyper/badge.svg?branch=master)](https://coveralls.io/r/hyperium/hyper?branch=master)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![crates.io](http://meritbadge.herokuapp.com/hyper)](https://crates.io/crates/hyper)

A Modern HTTP library for Rust.

### Documentation

- [Released](http://hyperium.github.io/hyper)
- [Master](http://hyperium.github.io/hyper/master)

## Overview

hyper is a fast, modern HTTP implementation written in and for Rust. It
is a low-level typesafe abstraction over raw HTTP, providing an elegant
layer over "stringly-typed" HTTP.

Hyper offers both an HTTP client and server which can be used to drive
complex web applications written entirely in Rust.

Be aware that hyper is still actively evolving towards 1.0, and is likely
to experience breaking changes before stabilising. The current area of
change is the movement towards async IO and refining the design around
that. You can also see the [1.0 issue milestone](https://github.com/hyperium/hyper/milestone/1).

The documentation is located at [http://hyperium.github.io/hyper](http://hyperium.github.io/hyper).
