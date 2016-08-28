% Server Guide

# Hello, World

Let's start off by creating a simple server to just serve a text response
of "Hello, World!" to every request.

```no_run
extern crate hyper;
use hyper::{Decoder, Encoder, HttpStream as Http, Next};
use hyper::server::{Server, Handler, Request, Response};

struct Text(&'static [u8]);

impl Handler<Http> for Text {
    fn on_request(&mut self, _req: Request<Http>) -> Next {
        Next::write()
    }

    fn on_request_readable(&mut self, _decoder: &mut Decoder<Http>) -> Next {
        Next::write()
    }

    fn on_response(&mut self, res: &mut Response) -> Next {
        use hyper::header::ContentLength;
        res.headers_mut().set(ContentLength(self.0.len() as u64));
        Next::write()
    }

    fn on_response_writable(&mut self, encoder: &mut Encoder<Http>) -> Next {
        encoder.write(self.0).unwrap(); // for now
        Next::end()
    }
}

fn main() {
    let addr = "127.0.0.1:0".parse().unwrap();
    let (listening, server) = Server::http(&addr).unwrap()
        .handle(|_| Text(b"Hello, World")).unwrap();

    println!("Listening on http://{}", listening);
    server.run()
}
```

There is quite a few concepts here, so let's tackle them one by one.

## Handler

The [`Handler`][Handler] is how you define what should happen during the lifetime
of an HTTP message. We've implemented it for the `Text`, defining what should
happen at each event during an HTTP message.

## Next

Every event in the [`Handler`][Handler] returns a [`Next`][Next]. This signals
to hyper what the `Handler` would wishes to do next, and hyper will call the
appropriate method of the `Handler` when the action is ready again.

So, in our "Hello World" server, you'll notice that when a request comes in, we
have no interest in the `Request` or its body. We immediately just wish to write
"Hello, World!", and be done. So, in `on_request`, we return `Next::write()`,
which tells hyper we wish to write the response.

After `on_response` is called, we ask for `Next::write()` again, because we
still need to write the response body. hyper knows that the next time the
transport is ready to be written, since it already called `on_response`, it
will call `on_response_writable`, which is where we can write the text body.

Once we're all done with the response, we can tell hyper to finish by returning
`Next::end()`. hyper will try to finish flushing all the output, and if the
conditions are met, it may try to use the underlying transport for another
request. This is also known as "keep-alive".

## Server

In the `main` function, a [`Server`][Server] is created that will utilize our
`Hello` handler. We use the default options, though you may wish to peruse
them, especially the `max_sockets` option, as it is conservative by default.

We pass a constructor closure to `Server.handle`, which constructs a `Handler`
to be used for each incoming request.

# Non-blocking IO

## Don't Panic

There is actually a very bad practice in the "Hello, World"  example. The usage
of `decoder.write(x).unwrap()` will panic if the write operation fails. A panic
will take down the whole thread, which means the event loop and all other
in-progress requests. So don't do it. It's bad.

What makes it worse, is that the write operation is much more likely to fail
when using non-blocking IO. If the write would block the thread, instead of
doing so, it will return an `io::Error` with the `kind` of `WouldBlock`. These
are expected errors.

## WouldBlock

Instead, we should inspect when there is a read or write error to see if the
`kind` was a `WouldBlock` error. Since `WouldBlock` is so common when using
non-blocking IO, the `Encoder` and `Decoder` provide `try_` methods that will
special case `WouldBlock`, allowing you to treat all `Err` cases as actual
errors.

Additionally, it's possible there was a partial write of the response body, so
we should probably change the example to keep track of it's progress. Can you
see how we should change the example to better handle these conditions?

This will just show the updated `on_response_writable` method, the rest stays
the same:

```no_run
# extern crate hyper;
# use hyper::{Encoder, HttpStream as Http, Next};

# struct Text(&'static [u8]);

# impl Text {
    fn on_response_writable(&mut self, encoder: &mut Encoder<Http>) -> Next {
        match encoder.try_write(self.0) {
            Ok(Some(n)) => {
                if n == self.0.len() {
                    // all done!
                    Next::end()
                } else {
                    // a partial write!
                    // with a static array, we can just move our pointer
                    // another option could be to store a separate index field
                    self.0 = &self.0[n..];
                    // there's still more to write, so ask to write again
                    Next::write()
                }
            },
            Ok(None) => {
                // would block, ask to write again
                Next::write()
            },
            Err(e) => {
                println!("oh noes, we cannot say hello! {}", e);
                // remove (kill) this transport
                Next::remove()
            }
        }
    }
# }

# fn main() {}
```

# Routing

What if we wanted to serve different messages depending on the URL of the
request? Say, we wanted to respond with "Hello, World!" to `/hello`, but
"Good-bye" with `/bye`. Let's adjust our example to do that.

```no_run
extern crate hyper;
use hyper::{Decoder, Encoder, HttpStream as Http, Next, StatusCode};
use hyper::server::{Server, Handler, Request, Response};

struct Text(StatusCode, &'static [u8]);

impl Handler<Http> for Text {
    fn on_request(&mut self, req: Request<Http>) -> Next {
        use hyper::RequestUri;
        let path = match *req.uri() {
            RequestUri::AbsolutePath { path: ref p, .. } => p,
            RequestUri::AbsoluteUri(ref url) => url.path(),
            // other 2 forms are for CONNECT and OPTIONS methods
            _ => ""
        };

        match path {
            "/hello" => {
                self.1 = b"Hello, World!";
            },
            "/bye" => {
                self.1 = b"Good-bye";
            },
            _ => {
                self.0 = StatusCode::NotFound;
                self.1 = b"Not Found";
            }
        }
        Next::write()
    }

#    fn on_request_readable(&mut self, _decoder: &mut Decoder<Http>) -> Next {
#        Next::write()
#    }

    fn on_response(&mut self, res: &mut Response) -> Next {
        use hyper::header::ContentLength;
        // we send an HTTP Status Code, 200 OK, or 404 Not Found
        res.set_status(self.0);
        res.headers_mut().set(ContentLength(self.1.len() as u64));
        Next::write()
    }

#    fn on_response_writable(&mut self, encoder: &mut Encoder<Http>) -> Next {
#        match encoder.try_write(self.1) {
#            Ok(Some(n)) => {
#                if n == self.1.len() {
#                    Next::end()
#                } else {
#                    self.1 = &self.1[n..];
#                    Next::write()
#                }
#            },
#            Ok(None) => {
#                Next::write()
#            },
#            Err(e) => {
#                println!("oh noes, we cannot say hello! {}", e);
#                Next::remove()
#            }
#        }
#    }
}

fn main() {
    let addr = "127.0.0.1:0".parse().unwrap();
    let (listening, server) = Server::http(&addr).unwrap()
        .handle(|_| Text(StatusCode::Ok, b"")).unwrap();

    println!("Listening on http://{}", listening);
    server.run()
}
```

# Waiting

More often than not, a server needs to something "expensive" before it can
provide a response to a request. This may be talking to a database, reading
a file, processing an image, sending its own HTTP request to another server,
or anything else that would impede the event loop thread. These sorts of actions
should be done off the event loop thread, when complete, should notify hyper
that it can now proceed. This is done by combining `Next::wait()` and the
[`Control`][Control].

## Control

The `Control` is provided to the `Handler` constructor; it is the argument we
have so far been ignoring. It's not needed if we don't ever need to wait a
transport. The `Control` is usually sent to a queue, or another thread, or
wherever makes sense to be able to use it when the "blocking" operations are
complete.

To focus on hyper instead of obscure blocking operations, we'll use this useless
sleeping thread to show it works.

```no_run
extern crate hyper;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use hyper::{Control, Next};

fn calculate_ultimate_question(rx: mpsc::Receiver<(Control, mpsc::Sender<&'static [u8]>)>) {
    thread::spawn(move || {
        while let Ok((ctrl, tx)) = rx.recv() {
            thread::sleep(Duration::from_millis(500));
            tx.send(b"42").unwrap();
            ctrl.ready(Next::write()).unwrap();
        }
    });
}

# fn main() {}
```

Our worker will spawn a thread that waits on messages. When receiving a message,
after a short nap, it will send back the "result" of the work, and wake up the
waiting transport with a `Next::write()` desire.

## Wait

Finally, let's tie in our worker thread into our `Text` handler:

```no_run
extern crate hyper;
use hyper::{Control, Decoder, Encoder, HttpStream as Http, Next, StatusCode};
use hyper::server::{Server, Handler, Request, Response};

use std::sync::mpsc;

struct Text {
    status: StatusCode,
    text: &'static [u8],
    control: Option<Control>,
    worker_tx: mpsc::Sender<(Control, mpsc::Sender<&'static [u8]>)>,
    worker_rx: Option<mpsc::Receiver<&'static [u8]>>,
}

impl Handler<Http> for Text {
    fn on_request(&mut self, req: Request<Http>) -> Next {
        use hyper::RequestUri;
        let path = match *req.uri() {
            RequestUri::AbsolutePath { path: ref p, .. } => p,
            RequestUri::AbsoluteUri(ref url) => url.path(),
            _ => ""
        };

        match path {
            "/hello" => {
                self.text = b"Hello, World!";
            },
            "/bye" => {
                self.text = b"Good-bye";
            },
            "/question" => {
                let (tx, rx) = mpsc::channel();
                // queue work on our worker
                self.worker_tx.send((self.control.take().unwrap(), tx)).unwrap();
                // save receive channel for response handling
                self.worker_rx = Some(rx);
                // tell hyper we need to wait until we can continue
                return Next::wait();
            }
            _ => {
                self.status = StatusCode::NotFound;
                self.text = b"Not Found";
            }
        }
        Next::write()
    }

#    fn on_request_readable(&mut self, _decoder: &mut Decoder<Http>) -> Next {
#        Next::write()
#    }
#

    fn on_response(&mut self, res: &mut Response) -> Next {
        use hyper::header::ContentLength;
        res.set_status(self.status);
        if let Some(rx) = self.worker_rx.take() {
            self.text = rx.recv().unwrap();
        }
        res.headers_mut().set(ContentLength(self.text.len() as u64));
        Next::write()
    }
#
#    fn on_response_writable(&mut self, encoder: &mut Encoder<Http>) -> Next {
#        unimplemented!()
#    }
}

# fn calculate_ultimate_question(rx: mpsc::Receiver<(Control, mpsc::Sender<&'static [u8]>)>) {
#    use std::sync::mpsc;
#    use std::thread;
#    use std::time::Duration;
#    thread::spawn(move || {
#        while let Ok((ctrl, tx)) = rx.recv() {
#            thread::sleep(Duration::from_millis(500));
#            tx.send(b"42").unwrap();
#            ctrl.ready(Next::write()).unwrap();
#        }
#    });
# }

fn main() {
    let (tx, rx) = mpsc::channel();
    calculate_ultimate_question(rx);
    let addr = "127.0.0.1:0".parse().unwrap();
    let (listening, server) = Server::http(&addr).unwrap()
        .handle(move |ctrl| Text {
            status: StatusCode::Ok,
            text: b"",
            control: Some(ctrl),
            worker_tx: tx.clone(),
            worker_rx: None,
        }).unwrap();

    println!("Listening on http://{}", listening);
    server.run()
}
```



[Control]: ../hyper/struct.Control.html
[Handler]: ../hyper/server/trait.Handler.html
[Next]: ../hyper/struct.Next.html
[Server]: ../hyper/server/struct.Server.html
