# Roadmap

## Goal

Align current hyper to the [hyper VISION](./VISION.md).

The VISION outlines a decision-making framework, use-cases, and general shape
of hyper. This roadmap describes the focus areas to continue to improve hyper
to look more like what is in the VISION.

## Focus Areas

While open source is not a company, open source can be guiding. We _can_ focus
attention to specific areas of improvement, which are based on conversations
with users, and prioritized by frequency and impact.

To that end, the following 4 areas are current focus of the project:

1. Documentation
2. `hyper-util`
3. HTTP/3
4. Observability

Each area benefits from having a top level description and goal, a place to
track progress, and a champion (or two) that helps push the effort.

### Documentation

hyper has stabilized, so investing in documentation is wise! The way it is used
won't change much, so documentation won't become outdated quickly. A tool that
people don't know how to use isn't helpful at all. This helps hyper be
**Understandable**.

The documentation focus area includes several different forms:

- The API docs as a reference.
- Examples as form of how-to.
- Website guides as tutorials.

Each of these could benefit from dedicated planning of their overall structure,
editing the content that already exists, and creating the rest that is sorely
missing.

### hyper-util

`hyper-util` serves two main purposes:

1. Provide useful patterns that build on top of hyper.
2. Explore, stabilize, and graduate some of those patterns into hyper itself.

To that end, there are several new features that can be worked on and iterated
on in `hyper-util` right now:

- New design for a higher-level `Client`.
- Breaking apart some patterns from `reqwest`, such as proxy helpers.
- Server automatic version detection.
- Improved builder patterns that make it easier to configure complicated
  options.

### HTTP/3

hyper has an HTTP/3 crate, `h3`, that is generic over any QUIC implementation,
similar to how hyper's HTTP/1 and HTTP/2 can be provided any IO transport. It
supports much of HTTP/3 already, and interoperates with most other
implementations. While some brave users have been trying it out the hard way
(such as reqwest), it's time to bring HTTP/3 to more users.

The aim is to eventually support `hyper::client::conn::http3` and
`hyper::server::conn::http3`.

To do so, work is needed:

- Harden the `h3` crate itself, such as fixing any straggling interop issues,
  and filling out the spec conformance tags we use for accountability.
- Proposal for (initially unstable) `hyper::rt::quic` integration, allowing
  people to bring their own QUIC.
- Write the `hyper::proto::http3` glue that translates hyper's connection
  patterns with the `h3` crate.

### Observability

It's extremely common once operating a service using hyper to want more
visibility in what exactly is happening. It's important to realize that there
are 3 concepts involved that frequently get conflated: events, tracing, and
metrics.

Some existing ways to get some of these:

- Unstable `tracing` integraton inside hyper.
- `tower_http::trace` which instruments outside of hyper, using `Service` and
  `Body`.

However, there are some events and metrics that are only known inside hyper,
and having official, stable support would be very helpful.

Some potential options would be:

- Stabilizing specific `tracing` events (blocked on the `tracing` crate
  stabilizing...)
- Provide a rudimentary, programmatic way to query metrics without another
  crate.
- Provide some sort of `hyper-metrics` helper.

## Beyond

The above are focus areas that are the most frequently asked for, and so have
the most attention. That doesn't mean that nothing else can be worked on.

Motivated individuals that want to help make other improvements are certainly
welcome!

