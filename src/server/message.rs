use std::marker::PhantomData;


use http::{self, Next};
use net::Transport;

use super::{Handler, request, response};

/// A MessageHandler for a Server.
///
/// This should be really thin glue between http::MessageHandler and
/// server::Handler, but largely just providing the proper types one
/// would expect in a Server Handler.
pub struct Message<H: Handler<T>, T: Transport> {
    handler: H,
    _marker: PhantomData<T>
}

impl<H: Handler<T>, T: Transport> Message<H, T> {
    pub fn new(handler: H) -> Message<H, T> {
        Message {
            handler: handler,
            _marker: PhantomData,
        }
    }
}

impl<H: Handler<T>, T: Transport> http::MessageHandler<T> for Message<H, T> {
    type Message = http::ServerMessage;

    fn on_incoming(&mut self, head: http::RequestHead) -> Next {
        trace!("on_incoming {:?}", head);
        let req = request::new(head);
        self.handler.on_request(req)
    }

    fn on_decode(&mut self, transport: &mut http::Decoder<T>) -> Next {
        self.handler.on_request_readable(transport)
    }

    fn on_outgoing(&mut self, head: &mut http::MessageHead<::status::StatusCode>) -> Next {
        let mut res = response::new(head);
        self.handler.on_response(&mut res)
    }

    fn on_encode(&mut self, transport: &mut http::Encoder<T>) -> Next {
        self.handler.on_response_writable(transport)
    }

    fn on_error(&mut self, error: ::Error) -> Next {
        self.handler.on_error(error)
    }

    fn on_remove(self, transport: T) {
        self.handler.on_remove(transport);
    }
}

