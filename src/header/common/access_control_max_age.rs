header! {
    #[doc="`Access-Control-Max-Age` header, part of"]
    #[doc="[CORS](http://www.w3.org/TR/cors/#access-control-max-age-response-header)"]
    #[doc=""]
    #[doc="The `Access-Control-Max-Age` header indicates how long the results of a"]
    #[doc="preflight request can be cached in a preflight result cache."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Access-Control-Max-Age = \"Access-Control-Max-Age\" \":\" delta-seconds"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `531`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, AccessControlMaxAge};"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(AccessControlMaxAge(1728000u32));"]
    #[doc="```"]
    (AccessControlMaxAge, "Access-Control-Max-Age") => [u32]

    test_access_control_max_age {
        test_header!(test1, vec![b"531"]);
    }
}
