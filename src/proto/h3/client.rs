use futures::{Future, Poll};
#[cfg(feature = "quinn-h3")]
use quinn_h3;

use body::Payload;
use ::proto::Dispatched;
use ::{Body, Request, Response};

type ClientRx<B> = ::client::dispatch::Receiver<Request<B>, Response<Body>>;

pub(crate) struct Client<B>
where
    B: Payload
{
    rx: ClientRx<B>,
    inner: quinn_h3::client::Client,
}

impl<B> Future for Client<B>
where
    B: Payload + 'static
{
    type Item = Dispatched;
    type Error = ::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {

        }
    }
}
