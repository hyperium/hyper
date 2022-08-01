use tokio::io::{AsyncRead, AsyncWrite};

use super::Service;
use crate::common::{task, Future, Poll};

// The same "trait alias" as tower::MakeConnection, but inlined to reduce
// dependencies.
pub trait MakeConnection<Target>: self::sealed::Sealed<(Target,)> {
    type Connection: AsyncRead + AsyncWrite;
    type Error;
    type Future: Future<Output = Result<Self::Connection, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>>;
    fn make_connection(&mut self, target: Target) -> Self::Future;
}

impl<S, Target> self::sealed::Sealed<(Target,)> for S where S: Service<Target> {}

impl<S, Target> MakeConnection<Target> for S
where
    S: Service<Target>,
    S::Response: AsyncRead + AsyncWrite,
{
    type Connection = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(self, cx)
    }

    fn make_connection(&mut self, target: Target) -> Self::Future {
        Service::call(self, target)
    }
}

mod sealed {
    pub trait Sealed<X> {}
}
