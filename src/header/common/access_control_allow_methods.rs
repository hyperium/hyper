use method::Method;

header! {
    #[doc="`Access-Control-Allow-Methods` header, part of"]
    #[doc="[CORS](http://www.w3.org/TR/cors/#access-control-allow-methods-response-header)"]
    #[doc=""]
    #[doc="The `Access-Control-Allow-Methods` header indicates, as part of the"]
    #[doc="response to a preflight request, which methods can be used during the"]
    #[doc="actual request."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Access-Control-Allow-Methods: \"Access-Control-Allow-Methods\" \":\" #Method"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `PUT, DELETE, XMODIFY`"]
    #[doc=""]
    #[doc="# Examples"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, AccessControlAllowMethods};"]
    #[doc="use hyper::method::Method;"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    AccessControlAllowMethods(vec![Method::Get])"]
    #[doc=");"]
    #[doc="```"]
    #[doc="```"]
    #[doc="use hyper::header::{Headers, AccessControlAllowMethods};"]
    #[doc="use hyper::method::Method;"]
    #[doc=""]
    #[doc="let mut headers = Headers::new();"]
    #[doc="headers.set("]
    #[doc="    AccessControlAllowMethods(vec!["]
    #[doc="        Method::Get,"]
    #[doc="        Method::Post,"]
    #[doc="        Method::Patch,"]
    #[doc="        Method::Extension(\"COPY\".to_owned()),"]
    #[doc="    ])"]
    #[doc=");"]
    #[doc="```"]
    (AccessControlAllowMethods, "Access-Control-Allow-Methods") => (Method)*

    test_access_control_allow_methods {
        test_header!(test1, vec![b"PUT, DELETE, XMODIFY"]);
    }
}
