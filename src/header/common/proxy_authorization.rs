
header! {
    #[doc="`Proxy-Authorization` header, defined in [RFC7235](http://tools.ietf.org/html/rfc7235#section-4.4)"]

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
