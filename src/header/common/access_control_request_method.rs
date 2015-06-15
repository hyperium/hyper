use method::Method;

header! {
    #[doc="`Access-Control-Request-Method` header, part of"]
    #[doc="[CORS](http://www.w3.org/TR/cors/#access-control-request-method-request-header)"]
    #[doc=""]
    #[doc="The `Access-Control-Request-Method` header indicates which method will be"]
    #[doc="used in the actual request as part of the preflight request."]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Access-Control-Request-Method: \"Access-Control-Request-Method\" \":\" Method"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `GET`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, AccessControlRequestMethod};"]
    #[doc="use hyper::method::Method;"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set(AccessControlRequestMethod(Method::Get));"]
    #[doc="```"]
    (AccessControlRequestMethod, "Access-Control-Request-Method") => [Method]

    test_access_control_request_method {
        test_header!(test1, vec![b"GET"]);
    }
}
