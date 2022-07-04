# hyper 1.0 Roadmap

## Goal

Align current hyper to the [hyper VISION][VISION].

The VISION outlines a decision-making framework, use-cases, and general shape
of hyper. This roadmap describes the currently known problems with hyper, and
then shows what changes are needed to make hyper 1.0 look more like what is in
the VISION.

## Known Issues


> **Note**: These known issues are as of hyper v0.14.x. After v1.0 is released,
ideally these issues will have been solved. Keeping this history may be helpful
to Future Us, though.

### Higher-level Client and Server problems

Both the higher-level `Client` and `Server` types have stability concerns.

For the `hyper::Server`:

- The `Accept` trait is complex, and too easy to get wrong. If used with TLS, a slow TLS handshake
  can affect all other new connections waiting for it to finish.
- The `MakeService<&IO>` is confusing. The bounds are an assault on the eyes.
- The `MakeService` API doesn't allow to easily annotate the HTTP connection with `tracing`.
- Graceful shutdown doesn't give enough control.


It's more common for people to simply use `hyper::server::conn` at this point,
than to bother with the `hyper::Server`.

While the `hyper::Client` is much easier to use, problems still exist:

- The whole `Connect` design isn't stable.
  - ALPN and proxies can provide surprising extra configuration of connections.
  - Some `Connect` implementations may wish to view the path, in addition to the scheme, host, and port.
  - Wants `runtime` feature
- The Pool could be made more general or composable. At the same time, more customization is
  desired, and it's not clear
how to expose it yet.


### Runtime woes

hyper has been able to support different runtimes, but it has sometimes awkward
default support for Tokio.

- The `runtime` cargo-feature isn't additive
- Built-in Tokio support can be confusing
- Executors and Timers
  - The `runtime` feature currently enables a few options that require a timer, such as timeouts and
    keepalive intervals. It implicitly relies on Tokio's timer context. This can be quite confusing.
- IO traits
  - Should we publicly depend on Tokio's traits?
  - `futures-io`?
    - Definitely nope.
    - Not stable. (0.3?)
    - No uninitialized memory.
  - Eventual `std` traits?
    - They've been in design for years.
    - We cannot base our schedule on them.
    - When they are stable, we can:
      - Provide a bridge in `hyper-util`.
      - Consider a 2.0 of hyper.
  - Define our own traits, provide util wrappers?

### Forwards-compatibility

There's a concern about forwards-compatibility. We want to be able to add
support for new HTTP features without needing a new major version. While most
of `http` and `hyper` are prepared for that, there's two potential problems.

- New frames on an HTTP stream (body)
   - Receiving a new frame type would require a new trait method
     - There's no way to implement a "receive unknown frame" that hyper doesn't know about.
   - Sending an unknown frame type would be even harder.
     - Besides being able to pass an "unknown" type through the trait, the user would need to be
       able to describe how that frame is encoded in HTTP/2/3.
- New HTTP versions
  - HTTP/3 will require a new transport abstraction. It's not as simple as just using some
    `impl AsyncRead + AsyncWrite`. While HTTP/2 bundled the concept of stream creation internally,
    and thus could be managed wholly on top of a read-write transport, HTTP/3 is different. Stream
    creation is shifted to the QUIC protocol, and HTTP/3 needs to be able to use that directly.
  - This means the existing `Connection` types for both client and server will not be able to
    accept a QUIC transport so we can add HTTP/3 support.

### Errors

It's not easy to match for specific errors.

The `Error::source()` can leak an internal dependency. For example, a
`hyper::Error` may wrap an `h2::Error`. Users can downcast the source at
runtime, and hyper internally changing the version of its `h2` dependency can
cause runtime breakage for users.

Formatting errors is in conflict with the current expected norm. The
`fmt::Display` implementation for `hyper::Error` currently prints its own
message, and then prints the message of any wrapped source error. The Errors
Working Group currently recommends that errors only print their own message
(link?). This conflict means that error "reporters", which crawl a source chain
and print each error, has a lot of duplicated information.

```
error fetching website: error trying to connect: tcp connect error: Connection refused (os error 61)
tcp connect error: Connection refused (os error 61)
Connection refused (os error 61)
```

While there is a good reason for why hyper's `Error` types do this, at the very
least, it _is_ unfortunate.

### You call hyper, or hyper calls you?

> Note: this problem space, of who calls whom, will be explored more deeply in
> a future article.

At times, it's been wondered whether hyper should call user code, or if user
code should call hyper. For instance, should a `Service` be called with a
request when the connection receives one, or should the user always poll for
the next request.

There's a similar question around sending a message body. Should hyper ask the
body for more data to write, or should the user call a `write` method directly?

These both get at a root topic about [write
observability](https://github.com/hyperium/hyper/issues/2181). How do you know
when a response, or when body data, has been written successfully? This is
desirable for metrics, or for triggering other side-effects. 

The `Service` trait also has some other frequently mentioned issues. Does
`poll_ready` pull its complexity weight for servers? What about returning
errors, what does that mean? Ideally users would turn all errors into
appropriate `http::Response`s. But in HTTP/2 and beyond, stream errors are
different from HTTP Server Error responses. Could the `Service::Error` type do
more to encourage best practices?

## Design

The goal is to get hyper closer to the [VISION][], using that to determine the
best way to solve the known issues above. The main thrust of the proposed
changes are to make hyper more **Flexible** and stable.

In order to keep hyper **Understandable**, however, the proposed changes *must*
be accompanied by providing utilities that solve the common usage patterns,
documentation explaining how to use the more flexible pieces, and guides on how
to reach for the `hyper-util`ity belt.

The majority of the changes are smaller and can be contained to the *Public
API* section, since they usually only apply to a single module or type. But the
biggest changes are explained in detail here.

### Split per HTTP version

The existing `Connection` types, both for the client and server, abstract over
HTTP version by requiring a generic `AsyncRead + AsyncWrite` transport type.
But as we figure out HTTP/3, that needs to change. So to prepare now, the
`Connection` types will be split up.

For example, there will now be `hyper::server::conn::http1::Connection` and
`hyper::server::conn::http2::Connection` types.

These specific types will still have a very similar looking API that, as the
VISION describes, provides **Correct** connection management as it pertains to
HTTP.

There will be still be a type to wrap the different versions. It will no longer
be generic over the transport type, to prepare for being able to wrap HTTP/3
connections. Exactly how it will wrap, either by using internal trait objects,
or an `enum Either` style, or using a `trait Connection` that each type
implements, is something to be determined. It's likely that this "auto" type
will start in `hyper-util`.

### Focus on the `Connection` level

As mentioned in the *Known Issues*, the higher-level `Client` and `Server` have
stability and complexity problems. Therefore, for hyper 1.0, the main API will
focus on the "lower-level" connection types. The `Client` and `Server` helpers
will be moved to `hyper-util`.

## Public API

### body

The `Body` struct is removed. Its internal "variants" are [separated into
distinct types](https://github.com/hyperium/hyper/issues/2345), and can start
in either `hyper-util` or `http-body-util`.

The exported trait `HttpBody` is renamed to `Body`.

A single `Body` implementation in `hyper` is the one provided by receiving
client responses and server requests. It has the name `Streaming`.

> **Unresolved**: Other names can be considered during implementation. Another
> option is to not publicly name the implementation, but return `Response<impl
Body>`s.

The `Body` trait will be experimented on to see about making it possible to
return more frame types beyonds just data and trailers.

> **Unresolved**: What exactly this looks like will only be known after
> experimentation.

### client

The high-level `hyper::Client` will be removed, along with the
`hyper::client::connect` module. They will be explored more in `hyper-util`.

As described in *Design*, the `client::conn` module will gain `http1` and
`http2` sub-modules, providing per-version `SendRequest`, `Connection`, and
`Builder` structs. An `auto` version can be explored in `hyper-util`.

### error

The `hyper::Error` struct remains in place.

All errors returned from `Error::source()` are made opaque. They are wrapped an
internal `Opaque` newtype that still allows printing, but prevents downcasting
to the internal dependency.

A new `hyper::error::Code` struct is defined. It is an opaque struct, with
associated constants defining various code variants.

> Alternative: define a non-exhaustive enum. It's not clear that this is
> definitely better, though. Keeping it an opaque struct means we can add
> secondary parts to the code in the future, or add bit flags, or similar
> extensions.

The purpose of `Code` is to provide an abstraction over the kind of error that
is encountered. The `Code` could be some behavior noticed inside hyper, such as
an incomplete HTTP message. Or it can be "translated" from the underlying
protocol, if it defines protocol level errors. For example, an
`h2::Reason::CANCEL`.

### rt

The `Executor` trait stays in here.

Define a new trait `Timer`, which describes a way for users to provide a source
of sleeping/timeout futures. Similar to `Executor`, a new generic is added to
connection builders to provide a `Timer`.

### server

The higher-level `hyper::Server` struct, its related `Builder`, and the
`Accept` trait are all removed.

The `AddrStream` struct will be completely removed, as it provides no value but
causes binary bloat.

Similar to `client`, and as describe in the *Design*, the `conn` modules will
be expanded to support `http1` and `http2` submodules. An `auto` version can be
explored in `hyper-util`.

### service

A vendored and simplified `Service` trait will be explored.

The error type for `Service`s used for a server will explore having the return
type changed from any error to one that can become a `hyper::error::Code`.

> **Unresolved**: Both of the above points are not set in stone. We will
> explore and decide if they are the best outcome during development.

The `MakeService` pieces will be removed.

### Cargo Features

Remove the `stream` feature. The `Stream` trait is not stable, and we cannot
depend on an unstable API.

Remove the `tcp` and `runtime` features. The automatic executor and timer parts
are handled by providing implementations of `Executor` and `Timer`. The
`connect` and `Accept` parts are also moving to `hyper-util`.

### Public Dependencies

- `http`
- `http-body`
- `bytes`

Cannot be public while "unstable":

- `tracing`

## `hyper-util`


### body

A channel implementation of `Body` that has an API to know when the data has
been successfully written is provided in `hyper_util::body::channel`.

### client

A `Pool` struct that implements `Service` is provided. It fills a similar role
as the previous `hyper::Client`.

> **Note**: The `Pool` might be something that goes into the `tower` crate
> instead. Or it might stay here as a slightly more specialized racing-connect
> pool. We'll find out as we go.

A `connect` submodule that mostly mirrors the existing `hyper::client::connect`
module is moved here. Connectors can be used as a source to provide `Service`s
used by the `Pool`.

### rt

We can provide Tokio-backed implementations of `Executor` and `Timer`.

### server

A `GracefulShutdown` helper is provided, to allow for similar style of graceful
shutdown as the previous `hyper::Server` did, but with better control.

# Appendix

## Unresolved Questions

There are some parts of the proposal which are not fully resolved. They are
mentioned in Design and API sections above, but also collected here for easy
finding. While they all have _plans_, they are more exploratory parts of the
API, and thus they have a higher possibility of changing as we implement them.

The goal is to have these questions resolved and removed from the document by
the time there is a [Release Candidate][timeline].

### Should there be `hyper::io` traits?

Depending on `tokio` just for `AsyncRead` and `AsyncWrite` is convenient, but
can be confusing for users integrating hyper with other runtimes. It also ties
our version directly to Tokio. We can consider having vendored traits, and
providing Tokio wrappers in `hyper-util`.

### Should returned body types be `impl Body`?

### How could the `Body` trait prepare for unknown frames?

We will experiment with this, and keep track of those experiments in a
dedicated issue. It might be possible to use something like this:

```rust
pub trait Body {
    type Data;
    fn poll_frame(..) -> Result<Option<Frame<Self::Data>>>;
}

pub struct Frame<T>(Kind<T>);

enum Kind<T> {
   Data(T),
   Trailers(HeaderMap),
   Unknown(Box<dyn FrameThingy>),
}
```

### Should there be a simplified `hyper::Service` trait, or should hyper depend on `tower-service`?

- There's still a few uncertain decisions around tower, such as if it should be
  changed to `async fn call`, and if `poll_ready` is the best way to handle
  backpressure.
- It's not clear that the backpressure is something needed at the `Server`
  boundary, thus meaning we should remove `poll_ready` from hyper.
- It's not 100% clear if we should keep the service pattern, or use a
  pull-based API. This will be explored in a future blog post.

## FAQ

### Why did you pick _that_ name? Why not this other better name?

Naming is hard. We certainly should solve it, but discussion for particular
names for structs and traits should be scoped to the specific issues. This
document is to define the shape of the library API.

### Should I publicly depend on `hyper-util`?

The `hyper-util` crate will not reach 1.0 when `hyper` does. Some types and
traits are being moved to `hyper-util`. As with any pre-1.0 crate, you _can_
publicly depend on it, but it is explicitly less stable.

In most cases, it's recommended to not publicly expose your dependency on
`hyper-util`. If you depend on a trait, such as used by the moved higher-level
`Client` or `Server`, it may be better for your users to define your own
abstraction, and then make an internal adapter.

### Isn't this making hyper harder?

We are making hyper more **flexible**. As noted in the [VISION][], most use
cases of hyper require it to be flexible. That _can_ mean that the exposed API
is lower level, and that it feels more complicated. It should still be
**understandable**.

But the hyper 1.0 effort is more than just the single `hyper` crate. Many
useful helpers will be migrated to a `hyper-util` crate, and likely improved in
the process. The [timeline][] also points out that we will have a significant
documentation push. While the flexible pieces will be in hyper to compose how
they need, we will also write guides for the [hyper.rs][] showing people how to
accomplish the most common tasks.

[timeline]: https://seanmonstar.com/post/676912131372875776/hyper-10-timeline
[VISION]: https://github.com/hyperium/hyper/pull/2772
[hyper.rs]: https://hyper.rs
