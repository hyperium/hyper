use header::ConnectionOption;

header! {
    #[doc="`Proxy-Connection` header"]

    (ProxyConnection, "Proxy-Connection") => (ConnectionOption)+

}

impl ProxyConnection {
    /// A constructor to easily create a `ProxyConnection: close` header.
    #[inline]
    pub fn close() -> ProxyConnection {
        ProxyConnection(vec![ConnectionOption::Close])
    }

    /// A constructor to easily create a `Connection: keep-alive` header.
    #[inline]
    pub fn keep_alive() -> ProxyConnection {
        ProxyConnection(vec![ConnectionOption::KeepAlive])
    }
}
