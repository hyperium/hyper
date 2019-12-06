use bytes::Buf;

use super::HttpBody;
use crate::common::buf::BufList;

/// Aggregate the data buffers from a body asynchronously.
///
/// The returned `impl Buf` groups the `Buf`s from the `HttpBody` without
/// copying them. This is ideal if you don't require a contiguous buffer.
pub async fn aggregate<T>(body: T) -> Result<impl Buf, T::Error>
where
    T: HttpBody,
{
    let mut bufs = BufList::new();

    futures_util::pin_mut!(body);
    while let Some(buf) = body.data().await {
        let buf = buf?;
        if buf.has_remaining() {
            bufs.push(buf);
        }
    }

    Ok(bufs)
}
