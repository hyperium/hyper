use unicase::UniCase;

header! {
    #[doc="`Access-Control-Request-Headers` header, part of"]
    #[doc="[CORS](http://www.w3.org/TR/cors/#access-control-request-headers-request-header)"]
    #[doc=""]
    #[doc="The `Access-Control-Request-Headers` header indicates which headers will"]
    #[doc="be used in the actual request as part of the preflight request."]
    #[doc="during the actual request."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Access-Control-Allow-Headers: \"Access-Control-Allow-Headers\" \":\" #field-name"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `accept-language, date`"]
    (AccessControlRequestHeaders, "Access-Control-Request-Headers") => (UniCase<String>)*

    test_access_control_request_headers {
        test_header!(test1, vec![b"accept-language, date"]);
    }
}
