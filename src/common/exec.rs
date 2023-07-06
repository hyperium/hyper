use std::future::Future;
use std::pin::Pin;

// If http2 is not enable, we just have a stub here, so that the trait bounds
// that *would* have been needed are still checked. Why?
//
// Because enabling `http2` shouldn't suddenly add new trait bounds that cause
// a compilation error.

pub struct H2Stream<F, B>(std::marker::PhantomData<(F, B)>);

impl<F, B, E> Future for H2Stream<F, B>
where
    F: Future<Output = Result<http::Response<B>, E>>,
    B: crate::body::Body,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        unreachable!()
    }
}
