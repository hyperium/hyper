//! dox
use http::{Extensions, Request, Response};

use self::sealed::{Ext, Sealed};

mod conn_info;

pub use self::conn_info::ConnectionInfo;


mod sealed {
    use http::Extensions;

    pub trait Sealed {
        fn ext(&self) -> &Extensions;
        fn ext_mut(&mut self) -> &mut Extensions;
    }

    pub trait Ext: Sealed {}
}

impl<B> Sealed for Request<B> {
    fn ext(&self) -> &Extensions {
        self.extensions()
    }

    fn ext_mut(&mut self) -> &mut Extensions {
        self.extensions_mut()
    }
}

impl<B> Ext for Request<B> {}

impl<B> Sealed for Response<B> {
    fn ext(&self) -> &Extensions {
        self.extensions()
    }

    fn ext_mut(&mut self) -> &mut Extensions {
        self.extensions_mut()
    }
}

impl<B> Ext for Response<B> {}
