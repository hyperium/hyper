use super::Service;
use crate::common::{task, Poll};
use crate::{Request, Uri};
use http::uri::{Authority, Parts, Scheme};

/// Wraps an HTTP service, injecting authority and scheme on every request.
#[derive(Debug)]
pub struct AddOrigin<S> {
    service: S,
    scheme: Scheme,
    authority: Authority,
}

impl<S> AddOrigin<S> {
    /// Creates a new `AddOrigin` middleware
    pub fn new(service: S, scheme: Scheme, authority: Authority) -> Self {
        AddOrigin {
            service,
            authority,
            scheme,
        }
    }

    /// Returns a reference to the HTTP scheme that is added to all requests
    pub fn scheme(&self) -> &Scheme {
        &self.scheme
    }

    /// Returns a reference to the HTTP authority that is added to all requests
    pub fn authority(&self) -> &Authority {
        &self.authority
    }
}

impl<S, ReqBody> Service<Request<ReqBody>> for AddOrigin<S>
where
    S: Service<Request<ReqBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), S::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> S::Future {
        let (mut head, body) = req.into_parts();
        let mut uri: Parts = head.uri.into();

        uri.scheme = Some(self.scheme.clone());
        uri.authority = Some(self.authority.clone());

        head.uri = Uri::from_parts(uri).expect("valid uri");

        let new_request = Request::from_parts(head, body);

        self.service.call(new_request)
    }
}
