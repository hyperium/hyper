# HIP-0002: `Body::poll_progress()`

- Authors: katelyn martin @cratelyn
- Created: 26-10-12
- PR: TBD

## Summary

This document proposes the introduction of a new `Body::poll_progress()`
trait method.

## Tenets

(List of tenets, values, guiding principles that are used to evaluate the
proposal.)

## Motivation

`http_body::Body` is a trait representing the streaming body of a request or
response. This trait is used throughout not only `hyper`, but also much of the
Rust ecosystem as the foundational interface representing the stream of data
in a message payload.

This trait is designed for use in asynchronous applications running inside of
runtimes like `tokio`, a common pattern within the Rust ecosystem for
performantly dealing with asynchronous I/O.

### Bodies are streams

Today, the signature of the `Body` trait is shown below. For the purpose of
this discussion we will elide the hinting methods `Body::is_end_stream()` and
`Body::size_hint()`.

```rust
pub trait Body {
    type Data: Buf;
    type Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>>;
}
```

This is worth comparing to the signature of `std::async_iter::AsyncIterator`
(rust-lang/rust#79024), which defines the concept of an asynchronous
iterator.

```rust
pub trait AsyncIterator {
    type Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>>;
}
```

Notice how `Body::poll_frame()` and `AsyncIterator::poll_next()` share the
same `Pin<&mut Self>` receiver, and both require a mutable reference to the
`std::task::Context` of an asynchronous task. Notice that both return a
`Poll<Option<T>>` value, but `Body::poll_frame()` differs in two ways: (1)
polling a body is a _fallible_ operation, and (2) a body yields
`http_body::Frame<T>`s, which may be either a chunk of data or a collection
of trailers.

To put this succinctly, `http_body::Body` is a fallible stream of frames,
intended for use in HTTP clients and servers.

### Bodies are not buffered streams

The idea for `Body::poll_progress()` is inspired in part by a problem that the
Rust project's async working group calls "_Barbara battles buffered streams_".
To briefly summarize, polling a collection of futures and processing their
results can be tricky, even for experienced Rust developers.

There is a wealth of previous literature on this topic that we can draw from,
but it's worth delineating how bodies _differ_ from something like
`FuturesUnordered` or `BufferUnordered`.

Most importantly, a body is _ordered_. Its contents are typically considered to
be consecutive chunks of a single object, each of which is a `Buf` of bytes.
These chunks are often coalesced using a combinator like
`http_body_util::combinators::Collect` into a single buffer, e.g. `Bytes`.

Relatedly, a body is _serial_. The individual chunks of a body are not, and can
not be, polled concurrently like the children contained within a collection
like `FuturesUnordered`. A body exposes a very similar public interface, but it
is not a composite of multiple concurrent asynchronous tasks.

### Futurelock is less applicable

"Futurelock" is an especially important part of the design space of
`AsyncIterator::poll_progress()`. Put briefly, asynchronous applications must
be wary of deadlocks caused by the interaction between cancellation and
contended resources.

Bodies and futures are similar in shape, but have subtly different concerns.
Futures are a general abstraction to represent the broader concept of
asynchrony, while bodies are narrowly focused upon streams of data being sent
across a network. Request and response bodies implementations do not typically
interact with locked resources.

### On Polling

If bodies are ordered, and not a collection of concurrent futures like those
that inspired `AsyncIterator::poll_progress()`, why would it benefit from a
mechanism like this? This excerpt from the documentation of
`std::future::Future::poll()` describes what occurs when an executor polls a
future that is not yet ready to yield a value:

> When a future is not ready yet, `poll` returns `Poll::Pending` and stores a
> clone of the `Waker` copied from the current `Context`. This `Waker` is then
> woken once the future can make progress. For example, a future waiting for a
> socket to become readable would call `.clone()` on the `Waker` and store it.
> When a signal arrives elsewhere indicating that the socket is readable,
> `Waker::wake` is called and the socket future's task is awoken. Once a task
> has been woken up, it should attempt to `poll` the future again, which may or
> may not produce a final value.

Now consider this snippet from Sean's post describing the motivation for
`Body::poll_progress()`, piping a body into a destination sink of some kind:

```rust
while let Some(frame) = body.frame().await? {
    // what if body (^) cancels while we wait here?
    dst.send(frame).await?;
}
```

> The destination might not have space, and so we’ll wait for it to get more.
> But the body could give up during that time. Since the task is only waiting
> on when the `dst` is ready, it could wait a significant amount of time and
> never notice that the body (source) has canceled.

To restate this succinctly: `Body` needs a way to propagate cancellation when
backpressure is being applied.

## Recommendation

Introduce a new method to `http_body::Body`, with the following signature:

```rust
fn poll_progress(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>
) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
}
```

When `Body::poll_frame()` returns `Poll::Pending`, consumers may then call
`poll_progress()`. Body middleware may use this method to indicate to their
consumers that a stream should be aborted.

<!-- markdownlint-disable-next-line heading-increment -->
#### The Rules of `poll_progress()`

A consumer is not required to call `poll_progress()` after `poll_frame()`
returns `Poll::Pending`. Even if `poll_progress()` returns `Poll::Pending`, the
consumer is not required to continue calling `poll_progress()` until it returns
`Poll::Ready()`.

After `poll_frame()` returns `Poll::Pending`, a consumer may continue to call
`poll_progress()` repeatedly, i.e. more than once, until it returns
`Poll::Ready(Ok(()))`. A consumer may continue to call `poll_progress()` even
after it returns `Poll::Ready(Ok(()))`.

When `poll_progress()` returns an error `Poll::Ready(Err(_))`, the consumer
should propagate this error as if it was an error returned by `poll_frame()`,
and discard the body. Consumers should not call `poll_frame()` after
`poll_progress()` returns an error.

A body _must_ allow calling `poll_progress()` after `poll_frame()` returns
`Poll::Pending`. A body is not required to support calling `poll_progress()`
until `poll_frame()` returns `Poll::Pending`.

A body is not required to support calling `poll_progress()` after
the body has been polled to completion, i.e. `poll_frame()` has returned
`Poll::Ready(None)`, or `is_end_stream()` has returned `true`.

<!-- markdownlint-disable-next-line heading-increment -->
#### `Body::poll_progress()` is a provided method

Above, this proposal suggests a default implementation for this trait method,
following what is outlined in the hyperium/http-body#90. It is worth taking a
moment to explicitly describe the tradeoff being made here, by opting to define
this as a provided method, rather than a required method.

Introducing a new provided method with a default implementation will not be a
breaking change. For a foundational crate like `http-body` that much of the
Rust ecosystem depends on, this is valuable, and avoids upgrade churn and
compatibility issues as different dependents upgrade to this new version of
the `Body` trait.

The downside of this is that users who wish to make use of this functionality
will need to manually audit their body combinators, types akin to those in
`http_body_util::combinators::*` that wrap an inner `B: Body`, for any missing
delegation.

For example, consider the following simple wrapper, that logs when
`Body::poll_frame()` is called via `tracing`:

```rust
struct Combinator<B> {
    inner: B,
}

impl<B: Body> Body for Combinator<B>
{
    type Data = B::Data;
    type Error = B::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        tracing::info!("poll_frame was called");
        this.inner.poll_frame(cx)
    }
}
```

This code, as written, would not call `<B as Body>::poll_progress()`. If the
inner `B` is a body whose implementation of `Body` contains meaningful logic
within its `poll_progress()`, this could be surprising to developers.

This is unfortunate, and requires care on the part of library authors and those
that wish to use `Body::poll_progress()` in their applications. However, this
is ultimately the same problem those that wish to make use of hints
(`is_end_stream()` or `size_hint()`) when consuming bodies in their
applications.

Finally, we may always promote a provided method into a required method at a
later date, in the unlikely event that these downsides turn out to outweigh the
compatibility issues that a 2.0 major release would entail. Introducing this
method with a default implementation is ultimately the conservative choice,
and one that can be reevaluated later if needed.

## Unresolved Questions

### Clarify `Poll::Pending` vs. `Poll::Ready(())`

We want a `poll_*`-shaped interface for this, in part so that we can perform
tasks such as polling timers that might otherwise be polled in `poll_frame`.

Above, we noted that when `Future::poll()` returns `Poll::Pending`, the
context's `Waker` is used to wake the task when the future can make further
progress.

This proposal would benefit from clarifying the semantic difference between
a return value of `Poll::Pending` or `Poll::Ready(())`.

### What about `Poll<Result<!, Self::Error>>`?

One choice could be to refrain from allowing `Poll::Ready(Ok(()))` at all, by
making use of the uninhabited "never" type. The `!` type has since been
stabilized (see rust-lang/rust#155499), and is stable as of Rust 1.100. This
was suggested
[multiple](https://github.com/hyperium/http-body/pull/90#issuecomment-2821998494)
[times](https://github.com/hyperium/http-body/pull/90#discussion_r1428381267)
during discussion in hyperium/http-body#90, and thus bears some examination.

```rust
fn poll_progress(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>
) -> Poll<Result<!, Self::Error>> {
    Poll::Pending
}
```

Here, polling a body for further progress can only return `Poll::Pending` or
`Poll::Ready(Err(E))`. This has some significant upsides. Most importantly,
this signature more clearly communicates the purpose of this interface, and
what consumers should expect to gain from calling it: eager propagation of
errors while `poll_frame()` is currently pending. Relatedly, this also
sidesteps a category of questions about the distinction between
`Poll::Ready(Ok(()))` and `Poll::Pending`. With this return type we need not
worry about whether `poll_progress()` is "fused", i.e. repeatedly callable
after yielding `Poll::Ready(Ok(()))`, for example.

The primary downside of using `!` for this would be disrupting hyper's minimum
supported rust version (MSRV). As a refresher, the MSRV policy is outlined
in `docs/MSRV.md`, and at time of writing is as follows:

> hyper's current policy is to always support a Rust version at least 6 months
> old. That is, a compiler version released within the last 6 months can
> compile hyper. It is possible that an older compiler can work, but that is
> not guaranteed. We try to increase the MSRV responsibly, only when a
> significant new feature is needed.
>
> The current MSRV is: **1.63**.

I would posit that this proposal _is_ a "significant new feature", given how
foundational `Body` is. This is a reasonable candidate to at least consider an
MSRV bump.

Rust 1.100, which stabilized `!`, was released on November 12th,
2026. Rust 1.100 version would accordingly become a candidate for potential use
in hyper on May 12th, 2026. The prototype in hyperium/http-body#90 was first
opened in March 2023, so this is also a fairly long-standing feature request
from hyper users. The choice of whether or not we elect to proceed with using
a return type of `Poll<Result<!, Self::Error>>` or
`Poll<Result<(), Self::Error>>` should be made on the basis of which type
provides the best long-term ergnomic benefits, rather than optimizing for MSRV
stability.

### Name Concerns

A variety of names for this method have been suggested over the last few years.
While I am not personally very subjectively drawn to any of these suggestions,
I want to catalogue them here (in roughly chronological order), for posterity:

- `poll_abort`
- `poll_healthy`
- `is_healthy`
- `poll_closed`
- `is_closed`
- `poll_canceled`
- `poll_frame_error`
- `poll_failure`

Above, we outlined some of the practical differences between `AsyncIterator`
and `Body`. Because these two interfaces both abstract over "streams",
adhering to a common convention of methods named `poll_progress` seems like a
strong point in favor of calling this method `poll_progress`.

There is fairly established consensus at this point that this mechanism should
have a `Pin<&mut Self>` receiver and accept a mutable reference to the current
`Context`, so that timers can be polled, and to facilitate other background
progress that might otherwise occur in `poll_frame`. With this in mind,
alternate names like `is_healthy` or `is_closed` would not follow the typical
API conventions of `is_*` methods having `&self` receivers and returning a
boolean.

Suggestions like `poll_frame_error` or `poll_failure` stemmed from questions
about error semantics, and I believe this can be addressed by providing clear
documentation about the rules of this interface.

## User Experience

## Implementation

## Testing Plan

(Optional: are there any special considerations for testing required?)

## Security Considerations

(What security risks might exist for the recommendation? Can we reduce them?
What should users consider when using it?)

## Alternatives

(List alternatives considered, and especially why they were not recommended.
This portion of "documenting the why" helps Future Us to see if anything has
changed since last considered.)

## References

- <https://docs.rs/http-body-util/latest/http_body_util/combinators/struct.Collect.html>
- <https://docs.rs/bytes/latest/bytes/buf/trait.Buf.html>
- <https://docs.rs/futures/latest/futures/prelude/trait.Stream.html>
- <https://github.com/rust-lang/rust/issues/79024>

<!-- markdownlint-disable-next-line heading-increment -->
#### Barbara Battles Buffered Streams

Other previous writing on the buffered stream problem can be found below:

- <https://github.com/rust-lang/wg-async/blob/master/src/vision/submitted_stories/status_quo/barbara_battles_buffered_streams.md>
- <https://rfd.shared.oxide.computer/rfd/0609>
- <https://tmandry.gitlab.io/blog/posts/for-await-buffered-streams/>
- <https://without.boats/blog/poll-progress/>
- <https://jacko.io/snooze.html>

<!-- markdownlint-disable-next-line heading-increment -->
#### Related Hyper Issues & PR's

- <https://seanmonstar.com/blog/body-poll-progress/>
- <https://github.com/hyperium/hyper/issues/3121>
- <https://github.com/hyperium/hyper/pull/3169>
- <https://github.com/hyperium/http-body/pull/90>

<!-- markdownlint-disable-next-line heading-increment -->
#### Rust RFC's

- <https://github.com/rust-lang/rfcs/pull/4019>
- <https://github.com/rust-lang/rfcs/pull/4020>
