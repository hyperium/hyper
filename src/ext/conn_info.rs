use std::net::SocketAddr;

use super::Ext;

/// dox
#[derive(Debug)]
pub struct ConnectionInfo {
    pub(crate) remote_addr: Option<SocketAddr>,
}

// The private type that gets put into extensions()
//
// The reason for the public and private types is to, for now, prevent
// a public API contract that crates could depend on. If the public type
// were inserted into the Extensions directly, a user could depend on
// `req.extensions().get::<ConnectionInfo>()`, and it's not clear that
// we want this contract yet.
#[derive(Copy, Clone, Default)]
struct ConnInfo {
    remote_addr: Option<SocketAddr>,
}

impl ConnectionInfo {
    /// dox
    pub fn get<E>(extend: &E) -> ConnectionInfo
    where
        E: Ext,
    {
        let info = extend
            .ext()
            .get::<ConnInfo>()
            .map(|&info| info)
            .unwrap_or_default();

        ConnectionInfo {
            remote_addr: info.remote_addr,
        }
    }

    /// dox
    pub(crate) fn set<E>(self, extend: &mut E)
    where
        E: Ext,
    {
        let info = ConnInfo {
            remote_addr: self.remote_addr,
        };

        extend.ext_mut().insert(info);
    }

    /// dox
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }
}

