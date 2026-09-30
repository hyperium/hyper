# HIP-0000: Sending Response Bodies from Files

- Authors: alexgleason
- Created: 2026-09-30
- PR: https://github.com/hyperium/hyper/pull/4215

## Summary

Let an HTTP/1 server send a response body straight from a file, so an IO that can do it uses a zero-copy system call like `sendfile(2)` and the file's bytes go from the page cache to the socket without being copied through the process.

The recommendation is additive and opt-in on both sides. A service attaches a `hyper::ext::SendFile { file, offset, len }` to a response that also carries an ordinary body holding the same bytes. An IO opts in by implementing two new provided methods on `rt::Write`: `supports_write_file` and `poll_write_file`. hyper uses the file only when both sides have opted in and the response's framing matches the file exactly. In every other case it sends the body as it does today.

## Tenets

These are drawn from hyper's [TENETS](../TENETS.md), in the same order of priority.

- **Correct:** a response is framed the same way whether its bytes come from the body or the file. If the file can't produce the bytes the head promised, the connection fails; it never hangs and it never sends a malformed message.
- **Fast:** large static bodies should cost less CPU per byte and reach higher throughput than today's best option, which is a `Bytes` over an mmap.
- **HTTP/\*:** this is about how hyper writes HTTP message bodies. It should not turn hyper into a general file server or add opinions about files beyond what framing needs.
- **Flexible:** a service should not need to know what kind of connection it is answering on. One response has to work over plain TCP, TLS, HTTP/2, and any user-provided IO. It should also be portable: nothing in hyper's API should be tied to one operating system.
- **Understandable:** nothing changes for anyone who doesn't opt in. No existing code changes behavior or stops compiling, and there is no new required bound on any existing API.

## Motivation

The fastest way to serve a static file over plain TCP is to have the kernel copy it straight from the page cache to the socket: `sendfile(2)` on Linux and the BSDs, `TransmitFile` on Windows, or `splice`/io_uring equivalents. Most mature HTTP servers do this (nginx `sendfile on`, Apache `EnableSendfile`, Go's `net/http` via `io.ReaderFrom`).

hyper can't. Every outgoing byte passes through `Body::Data: Buf` and then `rt::Write::poll_write(&[u8])`, so the bytes have to exist in userspace. Today a hyper server can:

- **Read the file into buffers.** One copy from the kernel into the process, one back out, and an allocation per chunk.
- **Hand hyper a `Bytes` over an mmap of the file.** This avoids the allocation, but the kernel still copies the bytes on `write`. It is also unsound if the file can change underneath, and a major page fault on a cold file stalls the async worker thread for as long as the disk takes. (Both points are from #3026.)
- **Read on a blocking thread ahead of time.** Avoids the stall, but pays the full copy.

#3026 asks for a way around this. It was marked as needing an RFC; seanmonstar wrote that it is "a desirable feature" and that it "can be added later … with some cleverness" rather than requiring a breaking change. This proposal is one attempt at that cleverness.

A prototype of this recommendation was measured serving one file over loopback with `oha`, 16 connections for 10 s. It compares a `Bytes` over an mmap against the same response plus a `SendFile`, with a Linux `sendfile(2)` IO. The machine was shared, so treat these as rough; the shape was consistent across runs.

| size   | mmap req/s | sendfile req/s | mmap CPU ms/GB | sendfile CPU ms/GB |
| ------ | ---------- | -------------- | -------------- | ------------------ |
| 16 KiB | 24,594     | 18,048         | 4,728          | 7,900              |
| 64 KiB | 14,240     | 15,209         | 2,028          | 2,358              |
| 4 MiB  | 832        | 1,383          | 1,372          | 724                |
| 64 MiB | 58         | 87             | 1,657          | 660                |

Large files get about 1.6x the throughput at 2 to 2.5x less CPU per byte. Small files get slower, because the head goes out as a separate write before the file (see [Unresolved Questions](#unresolved-questions)).

The same need exists for other fd-to-fd paths, such as `splice(2)` from a pipe or kTLS sockets, which can take `sendfile` too. This proposal covers regular files only, but tries not to rule the others out.

## Recommendation

### User Experience

#### Services

A service keeps returning an ordinary body. To offer a file for it, the service also attaches a `SendFile`:

```rust
use hyper::ext::SendFile;

let file: Arc<File> = /* opened once, shared across responses */;
let len = /* range length */;

let mut response = Response::new(body); // the same bytes, however the service likes
response.headers_mut().insert(CONTENT_LENGTH, len.into());
response.extensions_mut().insert(SendFile::new(file, offset, len));
```

The body is required and must hold the same bytes as the range. It is the fallback, and hyper uses it unchanged whenever the file can't be sent directly:

- the connection's IO doesn't support writing files, which includes every existing IO and every TLS stream,
- the connection is HTTP/2,
- the response isn't framed by a `Content-Length` exactly equal to `len`: it is chunked, or it has no body (a reply to `HEAD`, a `304`).

When the file is sent, the body is dropped without being polled. A body that reads the file lazily therefore costs nothing when it isn't needed.

hyper doesn't read the file's cursor and doesn't move it, so one `File` can back any number of concurrent responses. The file must not change while it is being sent.

```rust
/// hyper::ext
#[derive(Clone, Debug)]
pub struct SendFile { /* private */ }

impl SendFile {
    pub fn new(file: Arc<File>, offset: u64, len: u64) -> Self;
    pub fn file(&self) -> &Arc<File>;
    pub fn offset(&self) -> u64;
    pub fn len(&self) -> u64;
    pub fn is_empty(&self) -> bool;
}
```

`SendFile` is available with `http1` and `server`.

#### IO implementors

An IO opts in with two provided methods on `rt::Write`. They follow the precedent of `is_write_vectored`/`poll_write_vectored`:

```rust
pub trait Write {
    // ... existing methods ...

    /// Whether this writer implements `poll_write_file`. Defaults to `false`.
    fn supports_write_file(&self) -> bool { false }

    /// Write up to `len` bytes of `file` starting at `offset`, without passing
    /// them through a userspace buffer. Returns the number of bytes written;
    /// `0` means the file ended or the destination can't accept more.
    /// Must not move the file's cursor. Defaults to an `Unsupported` error.
    fn poll_write_file(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        file: &File,
        offset: u64,
        len: usize,
    ) -> Poll<io::Result<usize>> { /* Unsupported */ }
}
```

Both are forwarded through `Box`, `&mut`, and `Pin`, and through hyper's internal `Rewind`.

hyper ships no implementation. On Linux, a `tokio::net::TcpStream` wrapper needs about fifteen lines: wait with `poll_write_ready`, then call `libc::sendfile` inside `try_io(Interest::WRITABLE, ..)`. A TLS stream keeps the defaults and gets the body path. A kTLS stream could opt in.

### Implementation

All of this lives in the HTTP/1 server's dispatch and connection state. Clients and HTTP/2 are untouched.

1. When the dispatcher receives a response, it removes the `SendFile` from the head's extensions. It keeps it only if the connection is a server and `io.supports_write_file()` is true.
2. hyper writes the head exactly as it does today, picking the framing from the headers and body as usual. The `SendFile` has no effect on framing.
3. If the resulting encoder is `Content-Length` framing with exactly `len` bytes remaining, the connection records the file as the body source and drops the body receiver. Otherwise the file is discarded and the body is polled as normal.
4. While a file is the body source, the dispatcher calls `poll_write_file` instead of polling the body. Before the first call, the buffered head is flushed, even when `pipeline_flush` would otherwise defer it, because the file's bytes follow the head on the wire. The file offset advances and the encoder's remaining length drops by each write's size, across partial writes and `Pending`.
5. When the encoder reaches EOF, writing moves to `KeepAlive` or `Closed` exactly as it would after a polled body, and the connection continues.
6. If `poll_write_file` returns `0` before the range is done, the head has already promised more bytes than exist. The connection is closed with a body-write error.
7. If `poll_write_file` returns more than it was asked for, that is a broken IO. hyper fails the connection rather than trusting the count. (The prototype only has a `debug_assert!` for this; it should become a real check.)

In the prototype this is about 150 lines outside tests. There is one new `Option<SendFile>` in the connection state, and two small `Encoder` helpers: read the remaining length, and count bytes written outside `encode`.

### Testing Plan

hyper can't rely on a real `sendfile` in its test suite. Doing so would tie the tests to one OS and hide partial writes. The prototype uses a mock IO whose `poll_write_file` reads through `FileExt::read_at`, writes at most 4 bytes per call, and returns `Pending` between calls. That way every body takes many calls across many polls. The tests cover:

- a whole file, and a byte range from the middle of one,
- keep-alive, and pipelined requests with and without `pipeline_flush`,
- `HEAD`, `Connection: close`, and a client half-close,
- falling back to the body for an IO without support, a mismatched `Content-Length`, and a chunked body,
- a file shorter than its `Content-Length`, which fails the connection instead of hanging.

The existing suite runs unchanged, which covers "no behavior change without opt-in".

### Security Considerations

- **The file must match the promise.** The head is on the wire before the file is read. If the file is truncated in the meantime, the connection fails after a partial body. hyper can't do better once the head is sent, and neither can nginx. Clients see a short read, never a message boundary in the wrong place. If a file grows, only `len` bytes are sent.
- **The body and the file might disagree.** hyper doesn't compare them, and comparing would defeat the point. A service that attaches a file with different contents from its body will serve different bytes depending on the connection. This is documented as the caller's responsibility, like `Content-Length` itself.
- **Trusting the IO's byte count.** An IO that reports more bytes than requested could desynchronise framing, which becomes a smuggling risk on keep-alive connections. hyper must check `n <= len` and fail the connection otherwise (step 7 above).
- **Blocking on cold files.** On Linux, `sendfile` of a file that isn't in the page cache blocks the calling thread on disk I/O. That thread is an async worker, the same problem as a page fault on an mmap. hyper can't solve this; it's the IO's decision. Implementations can pre-fault with `readahead`/`posix_fadvise`, use `SF_NODISKIO` on FreeBSD, or use io_uring. This belongs in the trait docs.
- **Which file gets sent.** hyper sends exactly the range the service names and does no path handling. There is no new way for a client to pick a file.

## Alternatives

### Generalise `Body::Data` beyond `Buf` (#3026's sketch)

Give body chunks a length and let the IO consume them in its own way, for example `trait Io { type Data; fn poll_write_data(..) }`. This is the most general design. It covers splicing, pooled buffers, and the incoming side too.

Not recommended now. `Body::Data: Buf` is part of `http-body` 1.0, and hyper is generic over `B: Body`. Recognising a non-`Buf` chunk would need a new bound on `serve_connection` or a breaking `http-body` release. Either one is a breaking change across the ecosystem (tower, axum, tonic, and so on). The recommendation doesn't close this door. If `http-body` 2.0 ever generalises chunks, `SendFile` becomes one way to produce such a chunk and can be deprecated.

### A new `Frame` kind in `http-body`

For example `Frame::file(..)` next to `Frame::data` and `Frame::trailers`. This keeps the file in the body where it arguably belongs, and middleware could see it.

Not recommended. Adding a variant to `Frame` compiles, but it is a semantic break. Every existing consumer and body combinator (`map_frame`, `collect`, compression layers, tonic) assumes a frame is data or trailers, and would silently drop or mishandle file frames. It also moves the fallback decision into the body, which doesn't know what connection it's on.

### Recognise a file-backed body type directly

For example, hyper downcasts `B` or `B::Data` to a known `FileBody` type. Stable Rust has no specialization, and `Any` would need `'static` bounds hyper doesn't have. It would also make hyper own a file body type, which is less **HTTP/\*** and less **Flexible** than naming a range.

### A separate `WriteFile` trait instead of methods on `rt::Write`

This would be cleaner in isolation. But hyper only holds `I: rt::Read + rt::Write`, so it can't test for a second trait without specialization or a new bound on `serve_connection`, which would be breaking. `supports_write_file` plus a provided method mirrors how `is_write_vectored` already solves the same problem.

### No fallback body: `SendFile` replaces the body

A service would return an empty body and the file. This is simpler inside hyper, but then the service has to know whether this connection's IO supports files. That is TLS vs plain TCP vs HTTP/2, which is exactly what hyper abstracts away. A shared service would need two code paths. Requiring the body means one response is correct everywhere, and dropping it unpolled makes it nearly free.

### Take a raw fd (`AsFd`/`RawFd`) instead of `&File`

That is Unix-only. `&std::fs::File` is portable: Unix implementations get an fd from it, and Windows implementations get a `HANDLE` for `TransmitFile`. It also makes it clear the source is a regular file, which is what the length contract assumes. `splice` from a pipe or socket would need its own design.

### Do it outside hyper

- **An IO wrapper that intercepts body bytes.** For example, a body emits a sentinel chunk and the IO swaps in a `sendfile`. This is fragile: framing, chunked encoding, and buffering all sit between the body and the IO, and a user IO shouldn't depend on hyper's internal write batching.
- **Take over the connection after the head.** hyper has no way to hand the socket back mid-response outside of upgrades, and adding one is a bigger and riskier API than this.
- **Stay with mmap `Bytes`.** This is the status quo. It is measurably slower for large files (see [Motivation](#motivation)) and has the soundness and stall issues described there.

### HTTP/2

Not proposed. HTTP/2 splits a body into DATA frames under flow control, with multiplexing and usually TLS. A kernel copy of the payload between frame headers is possible in principle, but it lives in `h2` and deserves its own proposal. Under this recommendation HTTP/2 simply uses the body.

## Unresolved Questions

- **Sending the head and the file together.** The head currently goes out as its own write, which costs one extra syscall and, with `TCP_NODELAY`, one extra packet. That is why small files are slower in the numbers above. Options:
  - pass the buffered head into `poll_write_file` as a `&[IoSlice]` prefix. This maps to FreeBSD's `sendfile` headers, Windows' `TransmitFile` head buffer, and Linux `MSG_MORE`/`TCP_CORK`.
  - leave the signature alone and let IOs cork on their own.

  Changing the trait signature later is a breaking change, so this should be decided before the methods are stabilised. Should the first version take a prefix argument?
- **Where IO implementations should live.** `hyper_util::rt::TokioIo<T>` is generic over `T` and can't specialise for `TcpStream`. Should hyper-util grow a `TokioTcpIo` (or similar) with a `sendfile` implementation behind a feature, or should that live outside the hyperium crates?
- **Should this start as unstable?** It could go behind `hyper_unstable_send_file` (like `hyper_unstable_tracing`) until an IO implementation and the prefix question have seen real use.
- **A mismatched length: fall back silently, or error?** The recommendation falls back to the body, which is always correct but can hide a bug where the service meant to send the file. A `debug!`/`trace!` log is the likely middle ground.
- **Client requests.** The same mechanism would work for HTTP/1 request bodies, such as uploads. Is that wanted now, or left for later? Nothing here prevents it.

## References

- #3026: non-`Buf` body chunks (the originating issue, with seanmonstar's comment on adding this without breaking changes)
- #4214: prototype implementation of this recommendation, closed pending this HIP
- [`sendfile(2)`, Linux](https://man7.org/linux/man-pages/man2/sendfile.2.html)
- [`sendfile(2)`, FreeBSD](https://man.freebsd.org/cgi/man.cgi?query=sendfile&sektion=2) (headers/trailers, `SF_NODISKIO`)
- [`TransmitFile`, Windows](https://learn.microsoft.com/en-us/windows/win32/api/mswsock/nf-mswsock-transmitfile)
- [nginx `sendfile`](https://nginx.org/en/docs/http/ngx_http_core_module.html#sendfile)
- [Go `net/http` and `io.ReaderFrom`](https://pkg.go.dev/net#TCPConn.ReadFrom)
- rustls/rustls#198: kTLS support in rustls
