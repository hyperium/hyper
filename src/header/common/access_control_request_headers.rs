use unicase::UniCase;

header! {
    #[doc="`Access-Control-Request-Headers` header, part of"]
    #[doc="[CORS](www.w3.org/TR/cors/#access-control-request-headers-request-header)"]
    #[doc=""]
    #[doc="The `Access-Control-Request-Headers` header indicates which headers will"]
    #[doc="be used in the actual request as part of the preflight request."]
    #[doc="during the actual request."]
    (AccessControlRequestHeaders, "Access-Control-Request-Headers") => (UniCase<String>)*

    test_access_control_request_headers {}
}
