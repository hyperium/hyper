% Server Guide

## The `Handler`

```ignore,no_run
extern crate hyper;
use hyper::server::{Handler, Request, Response, Decoder, Encoder, Next, HttpStream as Http};

struct Hello;

impl Handler<Http> for Hello {
    fn on_request(&mut self, req: Request<Http>) -> Next {

    }

    fn on_request_readable(&mut self, decoder: &mut Decoder<Http>) -> Next {

    }

    fn on_response(&mut self, res: &mut Response) -> Next {

    }

    fn on_response_writable(&mut self, encoder: &mut Encoder<Http>) -> Next {

    }
}

# fn main() {}
```
