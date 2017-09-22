pub use http_types::Uri;
use http_types::uri::Parts;

pub fn scheme_and_authority(uri: &Uri) -> Option<Uri> {
    let mut parts = Parts::default();
    parts.authority = uri.authority().map(|s| s.parse().unwrap());
    parts.path_and_query = Some("".parse().unwrap());
    parts.scheme = uri.scheme().map(|s| s.parse().unwrap());

    Uri::from_parts(parts).ok()
}

pub fn origin_form(uri: &Uri) -> Uri {
    let parts = Parts::default();
    let mut path = uri.path().to_owned();

    if let Some(query) = uri.query() {
        path += "?";
        path += query;
    }

    Uri::from_parts(parts).unwrap()
}
