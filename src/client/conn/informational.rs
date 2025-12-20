//! Informational response handling for HTTP/2 client connections.
//!
//! This module provides callback-based handling of 1xx informational responses,
//! including 103 Early Hints, for HTTP/2 client connections.

use http::Response;
use std::fmt;
use std::sync::Arc;

/// A callback function for handling informational responses (1xx status codes).
///
/// This callback is invoked whenever the client receives an informational response
/// from the server, such as 103 Early Hints. The callback receives the complete
/// informational response including headers.
///
/// # Examples
///
/// ```rust
/// use hyper::client::conn::informational::InformationalCallback;
/// use http::{Response, StatusCode};
/// use std::sync::Arc;
///
/// let callback: InformationalCallback = Arc::new(|response: Response<()>| {
///     if response.status() == StatusCode::EARLY_HINTS {
///         println!("Received 103 Early Hints with {} headers",
///                  response.headers().len());
///         // Process Link headers for resource preloading
///         for link in response.headers().get_all("link") {
///             println!("Preload: {:?}", link);
///         }
///     }
/// });
/// ```
pub type InformationalCallback = Arc<dyn Fn(Response<()>) + Send + Sync>;

/// Configuration for informational response handling.
///
/// This struct allows configuring how informational responses should be handled
/// by the HTTP/2 client connection.
#[derive(Default)]
pub struct InformationalConfig {
    /// Optional callback for handling informational responses.
    /// If None, informational responses are ignored (current behavior).
    pub callback: Option<InformationalCallback>,
}

impl InformationalConfig {
    /// Creates a new informational configuration with no callback.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the callback for handling informational responses.
    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(Response<()>) + Send + Sync + 'static,
    {
        self.callback = Some(Arc::new(callback));
        self
    }

    /// Returns true if a callback is configured.
    pub fn has_callback(&self) -> bool {
        self.callback.is_some()
    }

    /// Invokes the callback if one is configured.
    ///
    /// This is a test helper method - in production code, the callback
    /// is extracted and called directly for better performance.
    #[cfg(test)]
    pub(crate) fn invoke_callback(&self, response: Response<()>) {
        if let Some(ref callback) = self.callback {
            callback(response);
        }
    }
}

impl fmt::Debug for InformationalConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InformationalConfig")
            .field("has_callback", &self.has_callback())
            .finish()
    }
}

impl Clone for InformationalConfig {
    fn clone(&self) -> Self {
        // Arc allows us to clone the callback
        Self {
            callback: self.callback.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_informational_config_creation() {
        let config = InformationalConfig::new();
        assert!(!config.has_callback());
    }

    #[test]
    fn test_informational_config_with_callback() {
        let called = Arc::new(Mutex::new(false));
        let called_clone = called.clone();

        let config = InformationalConfig::new().with_callback(move |_response| {
            *called_clone.lock().unwrap() = true;
        });

        assert!(config.has_callback());

        // Test callback invocation
        let mut response = Response::new(());
        *response.status_mut() = StatusCode::EARLY_HINTS;
        config.invoke_callback(response);

        assert!(*called.lock().unwrap());
    }

    #[test]
    fn test_informational_config_clone() {
        let config = InformationalConfig::new().with_callback(|_| {});
        assert!(config.has_callback());

        let cloned = config.clone();
        assert!(cloned.has_callback()); // Callback is cloned with Arc
    }

    #[test]
    fn test_early_hints_callback() {
        let received_links = Arc::new(Mutex::new(Vec::new()));
        let received_links_clone = received_links.clone();

        let config = InformationalConfig::new().with_callback(move |response| {
            if response.status() == StatusCode::EARLY_HINTS {
                for link in response.headers().get_all("link") {
                    received_links_clone
                        .lock()
                        .unwrap()
                        .push(link.to_str().unwrap().to_string());
                }
            }
        });

        // Simulate 103 Early Hints response
        let mut response = Response::new(());
        *response.status_mut() = StatusCode::EARLY_HINTS;
        response.headers_mut().insert(
            "link",
            "</style.css>; rel=preload; as=style".parse().unwrap(),
        );
        response.headers_mut().append(
            "link",
            "</script.js>; rel=preload; as=script".parse().unwrap(),
        );

        config.invoke_callback(response);

        let links = received_links.lock().unwrap();
        assert_eq!(links.len(), 2);
        assert!(links.contains(&"</style.css>; rel=preload; as=style".to_string()));
        assert!(links.contains(&"</script.js>; rel=preload; as=script".to_string()));
    }
}
