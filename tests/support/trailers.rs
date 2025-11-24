use bytes::Buf;
use futures_util::stream::Stream;
use http::header::HeaderMap;
use http_body::{Body, Frame};
use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// A body created from a [`Stream`].
    #[derive(Clone, Debug)]
    pub struct StreamBodyWithTrailers<S> {
        #[pin]
        stream: S,
        trailers: Option<HeaderMap>,
    }
}

impl<S> StreamBodyWithTrailers<S> {
    /// Create a new `StreamBodyWithTrailers`.
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            trailers: None,
        }
    }

    pub fn with_trailers(stream: S, trailers: HeaderMap) -> Self {
        Self {
            stream,
            trailers: Some(trailers),
        }
    }

    pub fn set_trailers(&mut self, trailers: HeaderMap) {
        self.trailers = Some(trailers);
    }
}

impl<S, D, E> Body for StreamBodyWithTrailers<S>
where
    S: Stream<Item = Result<Frame<D>, E>>,
    D: Buf,
{
    type Data = D;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let project = self.project();
        match project.stream.poll_next(cx) {
            Poll::Ready(Some(result)) => Poll::Ready(Some(result)),
            Poll::Ready(None) => match project.trailers.take() {
                Some(trailers) => Poll::Ready(Some(Ok(Frame::trailers(trailers)))),
                None => Poll::Ready(None),
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S: Stream> Stream for StreamBodyWithTrailers<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
