use unicase::UniCase;

header! {
    #[doc="`Access-Control-Allow-Headers` header, part of"]
    #[doc="[CORS](www.w3.org/TR/cors/#access-control-allow-headers-response-header)"]
    #[doc=""]
    #[doc="The `Access-Control-Allow-Headers` header indicates, as part of the"]
    #[doc="response to a preflight request, which header field names can be used"]
    #[doc="during the actual request."]
    #[doc=""]
    #[doc="# ABNF"]
    #[doc="```plain"]
    #[doc="Access-Control-Allow-Headers: \"Access-Control-Allow-Headers\" \":\" #field-name"]
    #[doc="```"]
    #[doc=""]
    #[doc="# Example values"]
    #[doc="* `accept-language, date`"]
    (AccessControlAllowHeaders, "Access-Control-Allow-Headers") => (UniCase<String>)*

    test_access_control_allow_headers {
        test_header!(test1, vec![b"accept-language, date"]);
    }
}
