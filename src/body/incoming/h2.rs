use std::task::{Context, Poll};

use bytes::Bytes;
use futures_util::ready;
use http_body::{Frame, SizeHint};

use crate::body::DecodedLength;
use crate::proto::h2::ping;

pub(super) struct H2Body {
    content_length: DecodedLength,
    data_done: bool,
    ping: ping::Recorder,
    recv: h2::RecvStream,
}

impl H2Body {
    pub(super) fn new(
        recv: h2::RecvStream,
        mut content_length: DecodedLength,
        ping: ping::Recorder,
    ) -> Self {
        // If the stream is already EOS, then the "unknown length" is clearly
        // actually ZERO.
        if !content_length.is_exact() && recv.is_end_stream() {
            content_length = DecodedLength::ZERO;
        }

        Self {
            data_done: false,
            ping,
            content_length,
            recv,
        }
    }

    pub(super) fn poll_frame(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, crate::Error>>> {
        let Self {
            ref mut data_done,
            ref ping,
            recv: ref mut h2,
            content_length: ref mut len,
        } = self;

        if !*data_done {
            match ready!(h2.poll_data(cx)) {
                Some(Ok(bytes)) => {
                    let _ = h2.flow_control().release_capacity(bytes.len());
                    len.sub_if(bytes.len() as u64);
                    ping.record_data(bytes.len());
                    return Poll::Ready(Some(Ok(Frame::data(bytes))));
                }
                Some(Err(e)) => {
                    return match e.reason() {
                        // These reasons should cause the body reading to stop, but not fail it.
                        // The same logic as for `Read for H2Upgraded` is applied here.
                        Some(h2::Reason::NO_ERROR) | Some(h2::Reason::CANCEL) => Poll::Ready(None),
                        _ => Poll::Ready(Some(Err(crate::Error::new_body(e)))),
                    };
                }
                None => {
                    *data_done = true;
                    // fall through to trailers
                }
            }
        }

        // after data, check trailers
        match ready!(h2.poll_trailers(cx)) {
            Ok(t) => {
                ping.record_non_data();
                Poll::Ready(Ok(t.map(Frame::trailers)).transpose())
            }
            Err(e) => Poll::Ready(Some(Err(crate::Error::new_h2(e)))),
        }
    }

    pub(super) fn is_end_stream(&self) -> bool {
        self.recv.is_end_stream()
    }

    pub(super) fn size_hint(&self) -> SizeHint {
        super::opt_len(self.content_length)
    }
}
