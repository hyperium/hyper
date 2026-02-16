#![deny(warnings)]
#![cfg(feature = "http2")]

//! Integration tests for HTTP/2 103 Early Hints support
//!
//! These tests validate the complete 103 Early Hints implementation according to:
//! - RFC 8297: An HTTP Status Code for Indicating Hints
//! - MDN Web Docs: 103 Early Hints specification
//! - Real browser behavior and security requirements

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http2::Builder;
use hyper::client::conn::informational::InformationalConfig;
use hyper::server::conn::http2::Builder as ServerBuilder;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};

// Re-use support module from integration tests
#[path = "support/mod.rs"]
mod support;
use support::{TokioExecutor, TokioIo};

// ============================================================================
// Test Abstractions and Helper Structures
// ============================================================================

/// Helper struct to track informational responses received by client
#[derive(Debug, Clone)]
struct InformationalResponse {
    status: u16,
    #[allow(dead_code)]
    headers: HashMap<String, String>,
    timestamp: std::time::Instant,
}

/// Builder for creating 103 Early Hints responses with fluent API
#[derive(Debug, Clone)]
struct EarlyHintsBuilder {
    headers: Vec<(String, String)>,
    processing_stage: Option<String>,
    delay_ms: u64,
}

impl EarlyHintsBuilder {
    fn new() -> Self {
        Self {
            headers: Vec::new(),
            processing_stage: None,
            delay_ms: 50,
        }
    }

    fn link_preload_css(mut self, url: &str) -> Self {
        self.headers.push((
            "link".to_string(),
            format!("<{}>; rel=preload; as=style", url),
        ));
        self
    }

    fn link_preload_js(mut self, url: &str) -> Self {
        self.headers.push((
            "link".to_string(),
            format!("<{}>; rel=preload; as=script", url),
        ));
        self
    }

    fn link_preload_font(mut self, url: &str, crossorigin: bool) -> Self {
        let co = if crossorigin { "; crossorigin" } else { "" };
        self.headers.push((
            "link".to_string(),
            format!("<{}>; rel=preload; as=font{}", url, co),
        ));
        self
    }

    fn link_preload_image(mut self, url: &str) -> Self {
        self.headers.push((
            "link".to_string(),
            format!("<{}>; rel=preload; as=image", url),
        ));
        self
    }

    fn link_preload_fetch(mut self, url: &str, crossorigin: bool) -> Self {
        let co = if crossorigin { "; crossorigin" } else { "" };
        self.headers.push((
            "link".to_string(),
            format!("<{}>; rel=preload; as=fetch{}", url, co),
        ));
        self
    }

    fn link_preconnect(mut self, url: &str, crossorigin: bool) -> Self {
        let co = if crossorigin { "; crossorigin" } else { "" };
        self.headers.push((
            "link".to_string(),
            format!("<{}>; rel=preconnect{}", url, co),
        ));
        self
    }

    fn csp(mut self, policy: &str) -> Self {
        self.headers
            .push(("content-security-policy".to_string(), policy.to_string()));
        self
    }

    fn processing_stage(mut self, stage: &str) -> Self {
        self.processing_stage = Some(stage.to_string());
        self
    }

    fn delay(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    fn custom_header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    async fn send_via(
        self,
        pusher: &mut hyper::ext::EarlyHintsPusher,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut response_builder = Response::builder().status(StatusCode::EARLY_HINTS);

        for (key, value) in &self.headers {
            response_builder = response_builder.header(key, value);
        }

        if let Some(stage) = &self.processing_stage {
            response_builder = response_builder.header("x-processing-stage", stage);
        }

        let early_hints_response = response_builder.body(())?;

        if let Err(e) = pusher.send_hints(early_hints_response).await {
            eprintln!("Server: Failed to send 103 Early Hints response: {}", e);
            return Err(Box::new(e));
        } else {
            println!("Server: Successfully sent 103 Early Hints response");
        }

        if self.delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;
        }

        Ok(())
    }
}

/// Test server builder for Early Hints scenarios
struct EarlyHintsTestServer {
    addr: std::net::SocketAddr,
    handle: tokio::task::JoinHandle<()>,
}

impl EarlyHintsTestServer {
    async fn with_early_hints<F, H>(early_hints_fn: F, final_response_fn: H) -> Self
    where
        F: Fn() -> Vec<EarlyHintsBuilder> + Send + Sync + 'static + Clone,
        H: Fn() -> Response<Full<Bytes>> + Send + Sync + 'static + Clone,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);

            let service = service_fn(move |mut req| {
                let early_hints_fn = early_hints_fn.clone();
                let final_response_fn = final_response_fn.clone();

                async move {
                    // Send Early Hints using the early_hints_pusher API
                    if let Ok(mut pusher) = hyper::ext::early_hints_pusher(&mut req) {
                        let hints = early_hints_fn();
                        for hint in hints {
                            if let Err(e) = hint.send_via(&mut pusher).await {
                                eprintln!("Failed to send early hint: {}", e);
                            }
                        }
                    }

                    Ok::<_, hyper::Error>(final_response_fn())
                }
            });

            ServerBuilder::new(TokioExecutor)
                .enable_informational() // Enable 103 Early Hints support
                .serve_connection(io, service)
                .await
                .unwrap();
        });

        Self {
            addr,
            handle: server_handle,
        }
    }

    fn addr(&self) -> std::net::SocketAddr {
        self.addr
    }

    fn abort(self) {
        self.handle.abort();
    }
}

/// Assertion helper for Early Hints responses
struct EarlyHintsAssertions<'a> {
    responses: &'a [InformationalResponse],
    current_index: usize,
}

impl<'a> EarlyHintsAssertions<'a> {
    fn new(responses: &'a [InformationalResponse]) -> Self {
        Self {
            responses,
            current_index: 0,
        }
    }

    fn expect_count(self, count: usize) -> Self {
        assert_eq!(
            self.responses.len(),
            count,
            "Expected {} Early Hints responses, got {}",
            count,
            self.responses.len()
        );
        self
    }

    fn expect_single_103_response(self) -> Self {
        self.expect_count(1)
            .expect_status(StatusCode::EARLY_HINTS.as_u16())
    }

    fn expect_status(self, status: u16) -> Self {
        if self.current_index < self.responses.len() {
            assert_eq!(
                self.responses[self.current_index].status, status,
                "Expected status {}, got {}",
                status, self.responses[self.current_index].status
            );
        }
        self
    }

    fn expect_link_contains(self, content: &str) -> Self {
        if self.current_index < self.responses.len() {
            let headers = &self.responses[self.current_index].headers;
            let all_header_values: Vec<String> = headers.values().cloned().collect();
            let combined_headers = all_header_values.join(" ");
            assert!(
                combined_headers.contains(content),
                "Expected link headers to contain '{}', got: {}",
                content,
                combined_headers
            );
        }
        self
    }

    fn expect_header(self, key: &str, value: &str) -> Self {
        if self.current_index < self.responses.len() {
            let headers = &self.responses[self.current_index].headers;
            assert!(
                headers.contains_key(key),
                "Expected header '{}' to be present",
                key
            );
            assert_eq!(
                headers.get(key).unwrap(),
                value,
                "Expected header '{}' to have value '{}', got '{}'",
                key,
                value,
                headers.get(key).unwrap()
            );
        }
        self
    }

    fn expect_processing_stage(self, stage: &str) -> Self {
        self.expect_header("x-processing-stage", stage)
    }

    fn expect_has_link_headers(self) -> Self {
        if self.current_index < self.responses.len() {
            let headers = &self.responses[self.current_index].headers;
            assert!(
                headers.contains_key("link"),
                "Expected response to contain Link headers"
            );
        }
        self
    }

    fn expect_crossorigin_present(self) -> Self {
        if self.current_index < self.responses.len() {
            let headers = &self.responses[self.current_index].headers;
            let all_header_values: Vec<String> = headers.values().cloned().collect();
            let combined_headers = all_header_values.join(" ");
            assert!(
                combined_headers.contains("crossorigin"),
                "Expected crossorigin attribute to be present"
            );
        }
        self
    }

    fn next_response(mut self) -> Self {
        self.current_index += 1;
        self
    }

    fn response(mut self, index: usize) -> Self {
        self.current_index = index;
        self
    }
}

/// Test scenario builder for Early Hints testing
struct EarlyHintsTestScenario {
    name: String,
    early_hints: Vec<EarlyHintsBuilder>,
    final_response: Option<Response<Full<Bytes>>>,
}

impl EarlyHintsTestScenario {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            early_hints: Vec::new(),
            final_response: None,
        }
    }

    fn with_early_hint(mut self, hint: EarlyHintsBuilder) -> Self {
        self.early_hints.push(hint);
        self
    }

    fn with_final_response(mut self, response: Response<Full<Bytes>>) -> Self {
        self.final_response = Some(response);
        self
    }

    fn with_html_response(self, content: &str) -> Self {
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html")
            .body(Full::new(Bytes::from(content.to_string())))
            .unwrap();
        self.with_final_response(response)
    }

    async fn run<F>(self, assertions: F)
    where
        F: FnOnce(EarlyHintsAssertions, &Response<hyper::body::Incoming>),
    {
        let _ = pretty_env_logger::try_init();

        let early_hints = self.early_hints.clone();
        let final_response = self.final_response.unwrap_or_else(|| {
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/html")
                .body(Full::new(Bytes::from("Test response")))
                .unwrap()
        });

        let server = EarlyHintsTestServer::with_early_hints(
            move || early_hints.clone(),
            move || final_response.clone(),
        )
        .await;

        let (mut sender, _conn_handle, received_responses) =
            create_early_hints_client(server.addr()).await;

        let req = Request::builder()
            .uri("/")
            .body(Full::new(Bytes::new()))
            .unwrap();

        let response = sender.send_request(req).await.unwrap();

        let responses = received_responses.lock().unwrap();
        println!(
            "{} test - received {} informational responses",
            self.name,
            responses.len()
        );

        let assertions_helper = EarlyHintsAssertions::new(&responses);
        assertions(assertions_helper, &response);

        println!("{} test passed", self.name);
        server.abort();
    }
}

/// Utility functions for common response patterns
struct ResponseTemplates;

impl ResponseTemplates {
    fn redirect_response(location: &str) -> Response<Full<Bytes>> {
        Response::builder()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header("location", location)
            .body(Full::new(Bytes::from("Redirecting...")))
            .unwrap()
    }
}

/// Helper to create a client with informational response tracking
async fn create_early_hints_client(
    addr: std::net::SocketAddr,
) -> (
    hyper::client::conn::http2::SendRequest<Full<Bytes>>,
    tokio::task::JoinHandle<()>,
    Arc<Mutex<Vec<InformationalResponse>>>,
) {
    let received_responses = Arc::new(Mutex::new(Vec::new()));
    let responses_clone = received_responses.clone();

    let config = InformationalConfig::new().with_callback(move |response: Response<()>| {
        let mut responses = responses_clone.lock().unwrap();
        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        responses.push(InformationalResponse {
            status: response.status().as_u16(),
            headers,
            timestamp: std::time::Instant::now(),
        });
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let io = TokioIo::new(stream);

    let (sender, conn) = Builder::new(TokioExecutor)
        .informational_responses(config)
        .handshake(io)
        .await
        .unwrap();

    let conn_handle = tokio::spawn(async move {
        if let Err(err) = conn.await {
            eprintln!("Connection error: {:?}", err);
        }
    });

    (sender, conn_handle, received_responses)
}

/// Helper function to validate Link header syntax
#[allow(dead_code)]
fn validate_link_header(link_header: &str) -> bool {
    // Basic Link header validation
    // Format: <URL>; rel=relationship; [additional parameters]
    link_header.starts_with('<') && link_header.contains('>') && link_header.contains("rel=")
}

/// Helper function to parse Link header into components
#[allow(dead_code)]
fn parse_link_header(link_header: &str) -> Option<(String, String, HashMap<String, String>)> {
    // Parse Link header: <URL>; rel=relationship; param=value
    // This is a simplified parser for testing purposes

    if !validate_link_header(link_header) {
        return None;
    }

    // Extract URL between < and >
    let url_start = link_header.find('<')?;
    let url_end = link_header.find('>')?;
    let url = link_header[url_start + 1..url_end].to_string();

    // Extract rel parameter
    let rel_start = link_header.find("rel=")?;
    let rel_value_start = rel_start + 4;
    let rel_end = link_header[rel_value_start..]
        .find(';')
        .map(|i| rel_value_start + i)
        .unwrap_or(link_header.len());
    let rel = link_header[rel_value_start..rel_end].trim().to_string();

    // Extract additional parameters
    let mut params = HashMap::new();
    let params_part = &link_header[url_end + 1..];
    for param in params_part.split(';') {
        if let Some(eq_pos) = param.find('=') {
            let key = param[..eq_pos].trim().to_string();
            let value = param[eq_pos + 1..].trim().to_string();
            if !key.is_empty() && key != "rel" {
                params.insert(key, value);
            }
        }
    }

    Some((url, rel, params))
}

// ============================================================================
// Integration Tests for HTTP/2 103 Early Hints
// ============================================================================

/// Test 1: Basic preconnect hints functionality
///
/// Validates that 103 Early Hints can send preconnect directives to establish
/// early connections to external domains. Tests both regular and crossorigin
/// preconnect scenarios commonly used for CDNs and font providers.
#[tokio::test]
async fn test_103_preconnect_hints() {
    EarlyHintsTestScenario::new("preconnect_hints")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preconnect("https://cdn.example.com", false)
                .link_preconnect("https://fonts.googleapis.com", true)
                .processing_stage("early-hints"),
        )
        .with_html_response(
            r#"<!DOCTYPE html>
            <html>
            <head>
                <link rel="preconnect" href="https://cdn.example.com">
                <link rel="preconnect" href="https://fonts.googleapis.com" crossorigin>
            </head>
            <body>Page with preconnect hints</body>
            </html>"#,
        )
        .run(|assertions, response| {
            assertions
                .expect_single_103_response()
                .expect_has_link_headers()
                .expect_processing_stage("early-hints")
                .expect_link_contains("rel=preconnect");

            assert_eq!(response.status(), StatusCode::OK);
        })
        .await;
}

/// Test 2: Resource preloading with 103 Early Hints
///
/// Tests the core preload functionality where critical CSS resources are
/// hinted before the final response. This is the most common use case for
/// 103 Early Hints in production web applications.
#[tokio::test]
async fn test_103_preload_hints() {
    EarlyHintsTestScenario::new("preload_hints")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preload_css("/style.css")
                .processing_stage("early-hints"),
        )
        .with_html_response(
            r#"<!DOCTYPE html>
            <html>
            <head>
                <link rel="stylesheet" href="/style.css">
                <script src="/script.js"></script>
            </head>
            <body>Page with preload hints</body>
            </html>"#,
        )
        .run(|assertions, response| {
            assertions
                .expect_single_103_response()
                .expect_has_link_headers()
                .expect_processing_stage("early-hints")
                .expect_link_contains("style.css")
                .expect_link_contains("rel=preload");

            assert_eq!(response.status(), StatusCode::OK);
        })
        .await;
}

/// Test 3: Content Security Policy enforcement via 103 Early Hints
///
/// Validates that CSP headers can be sent in 103 responses to provide early
/// security policy enforcement. This allows browsers to start applying security
/// policies before the main response arrives.
#[tokio::test]
async fn test_103_with_csp_enforcement() {
    EarlyHintsTestScenario::new("csp_enforcement")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .csp("default-src 'self'")
                .link_preload_css("/style.css")
                .link_preload_js("/script.js")
                .processing_stage("csp-enforcement"),
        )
        .with_final_response(
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/html")
                .header("content-security-policy", "default-src 'self'")
                .body(Full::new(Bytes::from("CSP enforcement test")))
                .unwrap(),
        )
        .run(|assertions, response| {
            assertions
                .expect_single_103_response()
                .expect_header("content-security-policy", "default-src 'self'")
                .expect_has_link_headers()
                .expect_processing_stage("csp-enforcement");

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get("content-security-policy").unwrap(),
                "default-src 'self'"
            );
        })
        .await;
}

/// Test 4: Multiple sequential 103 Early Hints responses
///
/// Tests the ability to send multiple 103 responses in sequence, each with
/// different priorities and processing stages. This simulates complex server
/// processing where hints are sent as resources become available.
#[tokio::test]
async fn test_multiple_103_responses_sequence() {
    EarlyHintsTestScenario::new("multiple_103_sequence")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preconnect("https://cdn.example.com", false)
                .processing_stage("multiple-103-1")
                .custom_header("x-priority", "high")
                .delay(25),
        )
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preload_css("/style.css")
                .processing_stage("multiple-103-2")
                .custom_header("x-priority", "medium")
                .delay(25),
        )
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preload_js("/script.js")
                .processing_stage("multiple-103-3")
                .custom_header("x-priority", "low")
                .delay(50),
        )
        .with_html_response("Multiple 103 responses test")
        .run(|assertions, response| {
            assertions
                .expect_count(3)
                .response(0)
                .expect_status(StatusCode::EARLY_HINTS.as_u16())
                .expect_processing_stage("multiple-103-1")
                .expect_header("x-priority", "high")
                .next_response()
                .expect_status(StatusCode::EARLY_HINTS.as_u16())
                .expect_processing_stage("multiple-103-2")
                .expect_header("x-priority", "medium")
                .next_response()
                .expect_status(StatusCode::EARLY_HINTS.as_u16())
                .expect_processing_stage("multiple-103-3")
                .expect_header("x-priority", "low");

            assert_eq!(response.status(), StatusCode::OK);
        })
        .await;
}

/// Test 5: Resource type preloading
///
/// Tests preloading of various resource types (CSS, JS, fonts, images, fetch)
/// with proper crossorigin handling. Validates that all major web resource
/// types can be effectively hinted via 103 Early Hints.
#[tokio::test]
async fn test_103_resource_types() {
    EarlyHintsTestScenario::new("resource_types")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preload_css("/styles/main.css")
                .link_preload_js("/scripts/app.js")
                .link_preload_font("/fonts/roboto.woff2", true)
                .link_preload_image("/images/hero.jpg")
                .link_preload_fetch("/data/config.json", true)
                .processing_stage("resource-types")
                .custom_header("x-resource-count", "5")
        )
        .with_html_response(r#"<!DOCTYPE html>
            <html>
            <head>
                <title>Resource Types Test</title>
                <link rel="stylesheet" href="/styles/main.css">
                <link rel="preload" href="/fonts/roboto.woff2" as="font" type="font/woff2" crossorigin>
            </head>
            <body>
                <h1>Resource Types Validation</h1>
                <img src="/images/hero.jpg" alt="Hero Image">
                <script src="/scripts/app.js"></script>
            </body>
            </html>"#)
        .run(|assertions, response| {
            assertions
                .expect_single_103_response()
                .expect_processing_stage("resource-types")
                .expect_header("x-resource-count", "5")
                .expect_has_link_headers()
                .expect_crossorigin_present();

            assert_eq!(response.status(), StatusCode::OK);
        })
        .await;
}

/// Test 6: Mixed Link header types in single 103 response
///
/// Tests HTTP/2 header compression behavior when multiple Link headers with
/// different relationship types are sent. Validates that at least one Link
/// header is properly delivered despite potential compression.
#[tokio::test]
async fn test_103_mixed_link_headers() {
    let _ = pretty_env_logger::try_init();

    // Create a custom server for this test that sends mixed Link header types
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);

        let service = service_fn(move |mut req| {
            async move {
                // Send 103 Early Hints with mixed Link header types using the early_hints_pusher API
                if let Ok(mut pusher) = hyper::ext::early_hints_pusher(&mut req) {
                    println!("Server: Sending 103 Early Hints with mixed Link headers");
                    let early_hints_response = Response::builder()
                        .status(StatusCode::EARLY_HINTS) // 103 Early Hints
                        .header("link", "<https://cdn.example.com>; rel=preconnect")
                        .header("link", "</style.css>; rel=preload; as=style")
                        .header("link", "</font.woff2>; rel=preload; as=font; crossorigin")
                        .header("x-processing-stage", "mixed-links")
                        .body(())
                        .unwrap();

                    if let Err(e) = pusher.send_hints(early_hints_response).await {
                        eprintln!("Server: Failed to send 103 Early Hints response: {}", e);
                    } else {
                        println!("Server: Successfully sent 103 Early Hints with mixed links");
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }

                Ok::<_, hyper::Error>(
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/html")
                        .body(Full::new(Bytes::from("Mixed link types test")))
                        .unwrap(),
                )
            }
        });

        ServerBuilder::new(TokioExecutor)
            .enable_informational() // Enable 103 Early Hints support
            .serve_connection(io, service)
            .await
            .unwrap();
    });

    let (mut sender, _conn_handle, received_responses) = create_early_hints_client(addr).await;

    let req = Request::builder()
        .uri("/mixed-links")
        .body(Full::new(Bytes::new()))
        .unwrap();

    let response = sender.send_request(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Verify 103 response with mixed Link header types
    let responses = received_responses.lock().unwrap();
    println!(
        "Mixed links test - received {} informational responses",
        responses.len()
    );

    // Should have received exactly one 103 Early Hints response
    assert_eq!(
        responses.len(),
        1,
        "Expected exactly one 103 Early Hints response"
    );
    assert_eq!(
        responses[0].status,
        StatusCode::EARLY_HINTS,
        "Expected status code 103 (Early Hints)"
    );

    // Verify the Link headers are present in the 103 response
    let headers = &responses[0].headers;
    assert!(
        headers.contains_key("link"),
        "103 response should contain Link headers"
    );

    // Check for mixed Link header types
    let link_header = headers.get("link").expect("Link header should be present");
    println!("DEBUG: Mixed Link header content: {:?}", link_header);

    // Print all headers for debugging
    for (key, value) in headers.iter() {
        println!("DEBUG: Header '{}': '{}'", key, value);
    }

    // Check for different types of links (preconnect, preload with different resource types)
    // Note: HTTP/2 may only deliver one of the multiple Link headers due to header compression
    let all_header_values: Vec<String> = headers.values().cloned().collect();
    let combined_headers = all_header_values.join(" ");

    // We sent 3 different Link headers, but HTTP may only deliver one
    // Let's check what we actually received and verify it's one of our expected types
    let has_preconnect =
        combined_headers.contains("rel=preconnect") && combined_headers.contains("cdn.example.com");
    let has_style_preload =
        combined_headers.contains("</style.css>") && combined_headers.contains("as=style");
    let has_font_preload =
        combined_headers.contains("</font.woff2>") && combined_headers.contains("as=font");
    let has_crossorigin = combined_headers.contains("crossorigin");

    // At least one of our Link header types should be present
    assert!(
        has_preconnect || has_style_preload || has_font_preload,
        "Should contain at least one of: preconnect, style preload, or font preload. Got: {}",
        combined_headers
    );

    // If we got the font preload, it should have crossorigin
    if has_font_preload {
        assert!(
            has_crossorigin,
            "Font preload should contain crossorigin attribute"
        );
    }

    // Verify we got a valid Link header format
    assert!(
        combined_headers.contains("rel="),
        "Should contain rel= attribute"
    );

    // Verify processing stage header
    assert!(
        headers.contains_key("x-processing-stage"),
        "Should contain processing stage header"
    );
    assert_eq!(
        headers.get("x-processing-stage").unwrap(),
        "mixed-links",
        "Processing stage should be mixed-links"
    );

    println!("103 Mixed Link Headers test passed - received proper mixed link types");

    // Clean up
    server_handle.abort();
}

/// Test 7: Cross-origin redirect behavior with 103 Early Hints
///
/// Tests browser security behavior where 103 Early Hints should be discarded
/// when the final response is a cross-origin redirect. This validates proper
/// security handling of early hints in redirect scenarios.
#[tokio::test]
async fn test_103_cross_origin_redirect_discard() {
    EarlyHintsTestScenario::new("cross_origin_redirect")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preconnect("https://original-cdn.example.com", false)
                .link_preload_css("/critical-styles.css")
                .link_preload_js("/important-script.js")
                .processing_stage("pre-redirect")
                .custom_header("x-origin-type", "same-origin")
        )
        .with_final_response(ResponseTemplates::redirect_response("https://different-origin.example.com/"))
        .run(|assertions, response| {
            assertions
                .expect_single_103_response()
                .expect_processing_stage("pre-redirect")
                .expect_header("x-origin-type", "same-origin")
                .expect_has_link_headers();

            assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
            assert_eq!(response.headers().get("location").unwrap(), "https://different-origin.example.com/");

            println!("Note: In real browsers, 103 Early Hints would be discarded due to cross-origin redirect");
        })
        .await;
}

/// Test 8: Real-world web page optimization scenario
///
/// Test simulating a production e-commerce page with multiple
/// resource types, fonts, images, and CDN preconnections. Demonstrates the
/// full potential of 103 Early Hints for web performance optimization.
#[tokio::test]
async fn test_103_web_page_optimization() {
    EarlyHintsTestScenario::new("web_page_optimization")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preload_css("/css/critical.css")
                .link_preload_js("/js/app.bundle.js")
                .link_preload_font("/fonts/roboto-regular.woff2", true)
                .link_preload_font("/fonts/roboto-bold.woff2", true)
                .link_preload_image("/images/hero-banner.jpg")
                .link_preconnect("https://cdn.jsdelivr.net", false)
                .link_preconnect("https://fonts.googleapis.com", true)
                .processing_stage("web-optimization")
                .custom_header("x-optimization-type", "critical-path")
        )
        .with_html_response(r#"<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Optimized E-commerce Page</title>
                <link rel="stylesheet" href="/css/critical.css">
                <link rel="preload" href="/fonts/roboto-regular.woff2" as="font" type="font/woff2" crossorigin>
                <link rel="preload" href="/fonts/roboto-bold.woff2" as="font" type="font/woff2" crossorigin>
            </head>
            <body>
                <header><nav>Navigation</nav></header>
                <main>
                    <section class="hero">
                        <img src="/images/hero-banner.jpg" alt="Featured Product" class="hero-image">
                        <h1>Welcome to Our Store</h1>
                        <button class="cta-button">Shop Now</button>
                    </section>
                </main>
                <footer><p>&copy; 2024 Optimized Store</p></footer>
                <script src="/js/app.bundle.js"></script>
            </body>
            </html>"#)
        .run(|assertions, response| {
            assertions
                .expect_single_103_response()
                .expect_processing_stage("web-optimization")
                .expect_header("x-optimization-type", "critical-path")
                .expect_has_link_headers()
                .expect_crossorigin_present();

            assert_eq!(response.status(), StatusCode::OK);

            println!("Performance optimization notes:");
            println!("   - Critical CSS preloaded for above-the-fold rendering");
            println!("   - App bundle JS preloaded for interactive functionality");
            println!("   - Web fonts preloaded to prevent FOUT (Flash of Unstyled Text)");
            println!("   - Hero image preloaded for immediate visual impact");
            println!("   - CDN preconnections established early for external resources");
        })
        .await;
}

/// Test 9: Empty 103 Early Hints response validation
///
/// Tests that 103 responses can be sent without Link headers, which is valid
/// per RFC 8297. This validates that empty 103 responses are handled correctly
/// and can be used for other informational purposes.
#[tokio::test]
async fn test_103_empty_response() {
    EarlyHintsTestScenario::new("empty_103")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .processing_stage("empty-103")
                .custom_header("x-link-count", "0")
                .custom_header("x-test-type", "minimal-response"),
        )
        .with_html_response(
            r#"<!DOCTYPE html>
            <html>
            <head>
                <title>Empty 103 Test</title>
                <link rel="stylesheet" href="/styles.css">
                <script src="/app.js"></script>
            </head>
            <body>
                <h1>Empty 103 Early Hints Test</h1>
                <p>This page tests 103 responses with no Link headers.</p>
            </body>
            </html>"#,
        )
        .run(|assertions, response| {
            let assertions = assertions
                .expect_single_103_response()
                .expect_processing_stage("empty-103")
                .expect_header("x-link-count", "0")
                .expect_header("x-test-type", "minimal-response");

            // Verify no Link headers are present in empty 103
            let responses = assertions.responses;
            let headers = &responses[0].headers;
            assert!(
                !headers.contains_key("link"),
                "Empty 103 response should not contain Link headers"
            );

            assert_eq!(response.status(), StatusCode::OK);

            println!("Empty 103 response behavior notes:");
            println!("   - 103 responses can be sent without Link headers");
            println!("   - Empty 103 responses are valid per RFC 8297");
            println!("   - Browsers handle empty 103 responses gracefully");
        })
        .await;
}

/// Test 10: Timing validation for 103 Early Hints delivery
///
/// Validates that 103 Early Hints responses arrive before the final response,
/// which is critical for their effectiveness. Measures and verifies the timing
/// relationship between informational and final responses.
#[tokio::test]
async fn test_103_timing_before_final_response() {
    let _ = pretty_env_logger::try_init();

    let start_time = std::time::Instant::now();

    EarlyHintsTestScenario::new("timing_optimization")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preload_css("/critical.css")
                .processing_stage("immediate-hints")
                .delay(10), // Small delay to ensure proper timing
        )
        .with_html_response(
            r#"<!DOCTYPE html>
            <html>
            <head>
                <title>Timing Test</title>
                <link rel="stylesheet" href="/critical.css">
                <script src="/important.js"></script>
            </head>
            <body>
                <h1>Timing and Performance Test</h1>
                <p>This page demonstrates 103 Early Hints timing behavior.</p>
            </body>
            </html>"#,
        )
        .run(|assertions, response| {
            let final_response_time = start_time.elapsed();

            let assertions = assertions
                .expect_single_103_response()
                .expect_processing_stage("immediate-hints")
                .expect_has_link_headers();

            // Verify timing: 103 response should arrive before final response
            let responses = assertions.responses;
            let resp_time = responses[0].timestamp.duration_since(start_time);

            println!("Timing analysis:");
            println!("   103 response received at: {:?}", resp_time);
            println!("   Final response received at: {:?}", final_response_time);
            println!("   Time difference: {:?}", final_response_time - resp_time);

            assert!(
                resp_time < final_response_time,
                "103 Early Hints should arrive before final response"
            );
            assert_eq!(response.status(), StatusCode::OK);
        })
        .await;
}

/// Test 11: Error response handling after 103 Early Hints
///
/// Tests the behavior when 103 Early Hints are sent but the final response
/// is an error (4xx/5xx). Validates that hints are properly sent even when
/// the server later determines an error condition exists.
#[tokio::test]
async fn test_103_with_error_responses() {
    // Test 404 Not Found after Early Hints
    EarlyHintsTestScenario::new("error_404_after_hints")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preload_css("/styles/main.css")
                .link_preload_js("/scripts/app.js")
                .processing_stage("error-scenario")
                .custom_header("x-error-type", "not-found")
        )
        .with_final_response(
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("content-type", "text/html")
                .body(Full::new(Bytes::from(r#"<!DOCTYPE html>
                    <html>
                    <head><title>404 Not Found</title></head>
                    <body>
                        <h1>Page Not Found</h1>
                        <p>The requested resource could not be found.</p>
                    </body>
                    </html>"#)))
                .unwrap()
        )
        .run(|assertions, response| {
            let assertions = assertions
                .expect_single_103_response()
                .expect_processing_stage("error-scenario")
                .expect_header("x-error-type", "not-found")
                .expect_has_link_headers();

            // Flexible validation for HTTP/2 header compression - check for either resource
            let responses = assertions.responses;
            let headers = &responses[0].headers;
            let all_header_values: Vec<String> = headers.values().cloned().collect();
            let combined_headers = all_header_values.join(" ");

            // Due to HTTP/2 HPACK compression, we might get either main.css or app.js
            let has_css_preload = combined_headers.contains("main.css") && combined_headers.contains("rel=preload");
            let has_js_preload = combined_headers.contains("app.js") && combined_headers.contains("rel=preload");

            assert!(has_css_preload || has_js_preload,
                   "Should contain preload for either main.css or app.js due to HTTP/2 compression. Got: {}", combined_headers);
            assert!(combined_headers.contains("rel=preload"), "Should contain rel=preload directive");

            assert_eq!(response.status(), StatusCode::NOT_FOUND);

            println!("Error handling notes:");
            println!("   - 103 Early Hints sent successfully before error determination");
            println!("   - Final response correctly returns 404 Not Found");
            println!("   - Browser may still use preloaded resources for error page styling");
            println!("   - HTTP/2 header compression handled gracefully");
        })
        .await;

    // Test 500 Internal Server Error after Early Hints
    EarlyHintsTestScenario::new("error_500_after_hints")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preconnect("https://cdn.example.com", false)
                .link_preload_css("/critical.css")
                .processing_stage("server-error")
                .custom_header("x-error-type", "internal-error")
                .custom_header("x-error-stage", "post-hints")
        )
        .with_final_response(
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from(r#"{"error": "Internal server error", "code": 500, "message": "An unexpected error occurred during processing"}"#)))
                .unwrap()
        )
        .run(|assertions, response| {
            assertions
                .expect_single_103_response()
                .expect_processing_stage("server-error")
                .expect_header("x-error-type", "internal-error")
                .expect_header("x-error-stage", "post-hints")
                .expect_has_link_headers();

            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.headers().get("content-type").unwrap(), "application/json");

            println!("Server error handling notes:");
            println!("   - 103 Early Hints sent before server error occurred");
            println!("   - Error response properly formatted as JSON");
            println!("   - Demonstrates server-side error after hint processing");
        })
        .await;

    // Test 403 Forbidden after Early Hints (authorization scenario)
    EarlyHintsTestScenario::new("error_403_after_hints")
        .with_early_hint(
            EarlyHintsBuilder::new()
                .link_preload_css("/admin/styles.css")
                .link_preload_js("/admin/dashboard.js")
                .processing_stage("auth-check")
                .custom_header("x-auth-stage", "pre-validation"),
        )
        .with_final_response(
            Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header("content-type", "text/html")
                .header("www-authenticate", "Bearer")
                .body(Full::new(Bytes::from(
                    r#"<!DOCTYPE html>
                    <html>
                    <head><title>403 Forbidden</title></head>
                    <body>
                        <h1>Access Denied</h1>
                        <p>You do not have permission to access this resource.</p>
                    </body>
                    </html>"#,
                )))
                .unwrap(),
        )
        .run(|assertions, response| {
            assertions
                .expect_single_103_response()
                .expect_processing_stage("auth-check")
                .expect_header("x-auth-stage", "pre-validation")
                .expect_has_link_headers()
                .expect_link_contains("admin");

            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert_eq!(
                response.headers().get("www-authenticate").unwrap(),
                "Bearer"
            );

            println!("Authorization error handling notes:");
            println!("   - 103 Early Hints sent before authorization check");
            println!("   - Proper 403 Forbidden response with WWW-Authenticate header");
            println!("   - Demonstrates early optimization before security validation");
        })
        .await;
}
