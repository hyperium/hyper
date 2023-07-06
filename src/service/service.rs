use std::future::Future;

/// An asynchronous function from a `Request` to a `Response`.
///
/// The `Service` trait is a simplified interface making it easy to write
/// network applications in a modular and reusable way, decoupled from the
/// underlying protocol.
///
/// # Functional
///
/// A `Service` is a function of a `Request`. It immediately returns a
/// `Future` representing the eventual completion of processing the
/// request. The actual request processing may happen at any time in the
/// future, on any thread or executor. The processing may depend on calling
/// other services. At some point in the future, the processing will complete,
/// and the `Future` will resolve to a response or error.
///
/// At a high level, the `Service::call` function represents an RPC request. The
/// `Service` value can be a server or a client.
pub trait Service<Request> {
    /// Responses given by the service.
    type Response;

    /// Errors produced by the service.
    type Error;

    /// The future response value.
    type Future: Future<Output = Result<Self::Response, Self::Error>>;

    /// Process the request and return the response asynchronously.
    /// call takes a &self instead of a mut &self because:
    /// - It prepares the way for async fn,
    ///   since then the future only borrows &self, and thus a Service can concurrently handle
    ///   multiple outstanding requests at once.
    /// - It's clearer that Services can likely be cloned
    /// - To share state across clones you generally need Arc<Mutex<_>>
    ///   that means you're not really using the &mut self and could do with a &self
    /// To see the discussion on this see: https://github.com/hyperium/hyper/issues/3040
    fn call(&self, req: Request) -> Self::Future;
}
