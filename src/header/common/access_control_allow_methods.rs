use method::Method;

header! {
    #[doc="`Access-Control-Allow-Methods` header, part of"]
    #[doc="[CORS](www.w3.org/TR/cors/#access-control-allow-methods-response-header)"]
    #[doc=""]
    #[doc="The `Access-Control-Allow-Methods` header indicates, as part of the"]
    #[doc="response to a preflight request, which methods can be used during the"]
    #[doc="actual request."]
    (AccessControlAllowMethods, "Access-Control-Allow-Methods") => (Method)*

    test_access_control_allow_methods {}
}
