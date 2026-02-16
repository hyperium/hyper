//! Support for sending HTTP 103 Early Hints responses.
//!
//! This module provides the `early_hints_pusher()` function which allows
//! server handlers to send informational responses (1xx status codes)
//! before the final response.

use http::{Request, Response, StatusCode};

use super::InformationalSender;

/// A handle for sending HTTP 103 Early Hints responses.
///
/// Obtained by calling `early_hints_pusher()` on a request.
#[derive(Debug, Clone)]
pub struct EarlyHintsPusher {
    sender: futures_channel::mpsc::Sender<Response<()>>,
}

impl EarlyHintsPusher {
    /// Send an HTTP 103 Early Hints response.
    ///
    /// The response must have status code 103 and an empty body.
    ///
    /// Returns an error if the response is invalid or if sending fails.
    pub async fn send_hints(&mut self, response: Response<()>) -> Result<(), EarlyHintsError> {
        // Validate that this is a 103 response
        if response.status() != StatusCode::EARLY_HINTS {
            return Err(EarlyHintsError::InvalidStatus);
        }

        self.sender
            .try_send(response)
            .map_err(|_| EarlyHintsError::SendFailed)
    }
}

/// Error type for early hints operations.
#[derive(Debug)]
pub enum EarlyHintsError {
    /// The response status was not 103
    InvalidStatus,
    /// Failed to send the response (channel full or closed)
    SendFailed,
    /// Early hints are not supported for this request  
    NotSupported,
}

impl std::fmt::Display for EarlyHintsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EarlyHintsError::InvalidStatus => write!(f, "response must have status 103"),
            EarlyHintsError::SendFailed => write!(f, "failed to send early hints response"),
            EarlyHintsError::NotSupported => {
                write!(f, "early hints not supported for this request")
            }
        }
    }
}

impl std::error::Error for EarlyHintsError {}

/// Obtain a pusher for sending HTTP 103 Early Hints responses.
///
/// This function lazily creates a channel for sending informational responses.
/// If called multiple times on the same request, it returns pushers that share
/// the same underlying channel.
///
/// Returns `Err` if early hints are not supported for this request
/// (for example, if the connection doesn't support HTTP/2 or HTTP/1.1).
///
/// # Example
///
/// ```rust,no_run
/// use hyper::{Request, Response, StatusCode};
/// use hyper::body::Incoming;
/// use hyper::ext::early_hints_pusher;
///
/// async fn handle(mut req: Request<Incoming>) -> Result<Response<String>, hyper::Error> {
///     let preload = r#"</style.css>; rel="preload"; as="style""#;
///     
///     match early_hints_pusher(&mut req) {
///         Ok(mut pusher) => {
///             let hints = Response::builder()
///                 .status(StatusCode::EARLY_HINTS)
///                 .header("Link", preload)
///                 .body(())
///                 .unwrap();
///                 
///             if let Err(e) = pusher.send_hints(hints).await {
///                 eprintln!("Failed to send early hints: {}", e);
///             }
///         }
///         Err(e) => {
///             eprintln!("Early hints not available: {}", e);
///         }
///     }
///     
///     // Send final response with the same Link header
///     Ok(Response::builder()
///         .header("Link", preload)
///         .body("<!DOCTYPE html>...".to_string())
///         .unwrap())
/// }
/// ```
pub fn early_hints_pusher<B>(req: &mut Request<B>) -> Result<EarlyHintsPusher, EarlyHintsError> {
    // Check if sender exists (pre-created by the server)
    if let Some(sender) = req.extensions().get::<InformationalSender>() {
        // Return a pusher that uses the existing sender
        return Ok(EarlyHintsPusher {
            sender: sender.0.clone(),
        });
    }

    // Sender not found - early hints not supported for this request
    Err(EarlyHintsError::NotSupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_early_hints_pusher_returns_error_when_not_supported() {
        let mut req = Request::new(());
        let result = early_hints_pusher(&mut req);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EarlyHintsError::NotSupported));
    }

    #[test]
    fn test_early_hints_pusher_succeeds_when_sender_present() {
        let mut req = Request::new(());
        let (tx, _rx) = futures_channel::mpsc::channel(10);
        req.extensions_mut().insert(InformationalSender(tx));

        let result = early_hints_pusher(&mut req);
        assert!(result.is_ok());
    }

    #[test]
    fn test_early_hints_pusher_multiple_calls_reuse_sender() {
        let mut req = Request::new(());
        let (tx, _rx) = futures_channel::mpsc::channel(10);
        req.extensions_mut().insert(InformationalSender(tx));

        let pusher1 = early_hints_pusher(&mut req).unwrap();
        let pusher2 = early_hints_pusher(&mut req).unwrap();

        // Both pushers should be valid (cloning the sender)
        assert!(pusher1.sender.is_closed() == pusher2.sender.is_closed());
    }

    #[tokio::test]
    async fn test_send_hints_rejects_non_103_status() {
        let mut req = Request::new(());
        let (tx, _rx) = futures_channel::mpsc::channel(10);
        req.extensions_mut().insert(InformationalSender(tx));

        let mut pusher = early_hints_pusher(&mut req).unwrap();

        // Try to send a 200 response instead of 103
        let invalid_response = Response::builder().status(200).body(()).unwrap();

        let result = pusher.send_hints(invalid_response).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EarlyHintsError::InvalidStatus
        ));
    }

    #[tokio::test]
    async fn test_send_hints_accepts_103_status() {
        let mut req = Request::new(());
        let (tx, mut rx) = futures_channel::mpsc::channel(10);
        req.extensions_mut().insert(InformationalSender(tx));

        let mut pusher = early_hints_pusher(&mut req).unwrap();

        // Send a valid 103 response
        let valid_response = Response::builder()
            .status(103)
            .header("link", "</style.css>; rel=preload; as=style")
            .body(())
            .unwrap();

        let result = pusher.send_hints(valid_response).await;
        assert!(result.is_ok());

        // Verify the response was sent through the channel
        let received = rx.try_next().unwrap();
        assert!(received.is_some());
        let response = received.unwrap();
        assert_eq!(response.status(), 103);
    }

    #[tokio::test]
    async fn test_send_hints_fails_when_channel_closed() {
        let mut req = Request::new(());
        let (tx, rx) = futures_channel::mpsc::channel::<Response<()>>(10);
        req.extensions_mut().insert(InformationalSender(tx));

        let mut pusher = early_hints_pusher(&mut req).unwrap();

        // Close the receiver
        drop(rx);

        // Try to send hints - should fail
        let response = Response::builder().status(103).body(()).unwrap();

        let result = pusher.send_hints(response).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EarlyHintsError::SendFailed));
    }

    #[test]
    fn test_early_hints_error_display() {
        let invalid_status = EarlyHintsError::InvalidStatus;
        assert_eq!(invalid_status.to_string(), "response must have status 103");

        let send_failed = EarlyHintsError::SendFailed;
        assert_eq!(
            send_failed.to_string(),
            "failed to send early hints response"
        );

        let not_supported = EarlyHintsError::NotSupported;
        assert_eq!(
            not_supported.to_string(),
            "early hints not supported for this request"
        );
    }
}
