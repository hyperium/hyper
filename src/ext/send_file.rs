use std::fs::File;
use std::sync::Arc;

/// A range of a file to send as an HTTP/1 response body without copying it
/// through the process.
///
/// Insert a `SendFile` into the extensions of an `http::Response`, and hyper's
/// HTTP/1 server will write that range of the file to the connection with
/// [`Write::poll_write_file`](crate::rt::Write::poll_write_file) instead of
/// polling the response body. On a plain TCP socket this lets the IO use a
/// zero-copy system call like Linux's `sendfile(2)`, which hands the file's
/// pages from the kernel's page cache to the socket.
///
/// The response body is still required, and must hold the same bytes. It is
/// the fallback, used whenever the file can't be sent directly:
///
/// - the connection's IO doesn't support it (its
///   [`supports_write_file`](crate::rt::Write::supports_write_file) returns
///   `false`, as it does for TLS streams and by default),
/// - the connection is HTTP/2,
/// - or the response isn't framed with a `Content-Length` equal to `len`:
///   it is chunked, or has no body at all (a reply to `HEAD`, a `304`).
///
/// When the file is sent, the body is dropped without being polled, so a body
/// that reads the file lazily costs nothing.
///
/// The head is flushed on its own before the file is handed to the IO, so a
/// response sent this way costs at least one more write than a buffered one.
/// For small bodies that can outweigh the copy it saves; it pays off for
/// large ones.
///
/// The file must not be modified while it is being sent. hyper reads it by
/// offset and never moves its cursor, so one `File` can be shared by any
/// number of responses at once.
///
/// # Example
///
/// ```
/// # fn doc(file: std::sync::Arc<std::fs::File>, bytes: bytes::Bytes) {
/// use http_body_util::Full;
/// use hyper::ext::SendFile;
///
/// let len = bytes.len() as u64;
/// let mut response = http::Response::new(Full::new(bytes));
/// response.extensions_mut().insert(SendFile::new(file, 0, len));
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct SendFile {
    pub(crate) file: Arc<File>,
    pub(crate) offset: u64,
    pub(crate) len: u64,
}

impl SendFile {
    /// Send `len` bytes of `file`, starting at `offset`.
    pub fn new(file: Arc<File>, offset: u64, len: u64) -> Self {
        SendFile { file, offset, len }
    }

    /// The file to send from.
    pub fn file(&self) -> &Arc<File> {
        &self.file
    }

    /// Where in the file the body starts.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// How many bytes of the file make up the body.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Whether the body is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
