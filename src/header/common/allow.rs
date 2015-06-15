use method::Method;

header! {
    #[doc="`Allow` header, defined in [RFC7231](http://tools.ietf.org/html/rfc7231#section-7.4.1)"]
    #[doc=""]
    #[doc="The `Allow` header field lists the set of methods advertised as"]
    #[doc="supported by the target resource.  The purpose of this field is"]
    #[doc="strictly to inform the recipient of valid request methods associated"]
    #[doc="with the resource."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Allow = #method"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `GET, HEAD, PUT`"]
    #[doc="* `OPTIONS, GET, PUT, POST, DELETE, HEAD, TRACE, CONNECT, PATCH, fOObAr`"]
    #[doc="* ``"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Allow};"]
    #[doc="use hyper::method::Method;"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    Allow(vec![Method::Get])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, Allow};"]
    #[doc="use hyper::method::Method;"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    Allow(vec!["]
    #[doc="        Method::Get,"]
    #[doc="        Method::Post,"]
    #[doc="        Method::Patch,"]
    #[doc="        Method::Extension(\"COPY\".to_owned()),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    (Allow, "Allow") => (Method)*

    test_allow {
        // From the RFC
        test_header!(
            test1,
            vec![b"GET, HEAD, PUT"],
            Some(HeaderField(vec![Method::Get, Method::Head, Method::Put])));
        // Own tests
        test_header!(
            test2,
            vec![b"OPTIONS, GET, PUT, POST, DELETE, HEAD, TRACE, CONNECT, PATCH, fOObAr"],
            Some(HeaderField(vec![
                Method::Options,
                Method::Get,
                Method::Put,
                Method::Post,
                Method::Delete,
                Method::Head,
                Method::Trace,
                Method::Connect,
                Method::Patch,
                Method::Extension("fOObAr".to_owned())])));
        test_header!(
            test3,
            vec![b""],
            Some(HeaderField(Vec::<Method>::new())));
    }
}

bench_header!(bench,
    Allow, { vec![b"OPTIONS,GET,PUT,POST,DELETE,HEAD,TRACE,CONNECT,PATCH,fOObAr".to_vec()] });
