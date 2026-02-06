//! HTTP/2 server demonstrating 103 Early Hints
//!
//! This example shows the recommended approach: 103 Early Hints.
//!
//! Run with:
//! ```
//! cargo run --example http2_early_hints --features full
//! ```

use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::time::Instant;

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Incoming as IncomingBody;
use hyper::ext::early_hints_pusher;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use tokio::net::TcpListener;
use tokio_rustls::rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConfig,
};
use tokio_rustls::TlsAcceptor;

#[path = "../benches/support/mod.rs"]
mod support;
use support::{TokioExecutor, TokioIo};

/// Load certificates from provided files
fn load_certificates() -> Result<
    (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Read certificate file
    let cert_pem = fs::read_to_string("/tmp/cert.txt")?;

    // Parse certificate chain
    let mut certs = Vec::new();
    for cert in rustls_pemfile::certs(&mut cert_pem.as_bytes()) {
        certs.push(cert?);
    }

    // Read private key file
    let key_pem = fs::read_to_string("/tmp/key.txt")?;

    // Parse private key
    let mut key_reader = key_pem.as_bytes();
    let key =
        rustls_pemfile::private_key(&mut key_reader)?.ok_or("No private key found in key file")?;

    Ok((certs, key))
}

/// Generate a self-signed certificate for testing (fallback)
fn generate_self_signed_cert() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    use rcgen::{Certificate as RcgenCert, CertificateParams, DistinguishedName};

    let mut params = CertificateParams::new(vec!["localhost".to_string()]);
    params.distinguished_name = DistinguishedName::new();

    let cert = RcgenCert::from_params(params).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let private_key_der = cert.serialize_private_key_der();

    (
        vec![CertificateDer::from(cert_der)],
        PrivateKeyDer::try_from(private_key_der).unwrap(),
    )
}

/// HTTP service demonstrating 103 Early Hints
async fn handle_request(
    mut req: Request<IncomingBody>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    println!("Received request: {} {}", req.method(), req.uri());

    // Handle static resources that we hinted about
    match path {
        // CSS Resources
        "/css/critical.css" => {
            let css_content = r#"
/* Critical CSS - Above the fold styling */
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: 'Roboto', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    line-height: 1.6;
    color: #333;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    min-height: 100vh;
}

.hero {
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    color: white;
    text-shadow: 2px 2px 4px rgba(0,0,0,0.3);
}

.hero h1 {
    font-size: 4rem;
    font-weight: 700;
    margin-bottom: 1rem;
    animation: fadeInUp 1s ease-out;
}

@keyframes fadeInUp {
    from {
        opacity: 0;
        transform: translateY(30px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}
"#;
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/css")
                .header("cache-control", "public, max-age=31536000")
                .body(Full::new(Bytes::from(css_content)))
                .unwrap());
        }

        "/css/layout.css" => {
            let css_content = r#"
/* Layout CSS - Page structure and components */
.container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 0 2rem;
}

.navbar {
    position: fixed;
    top: 0;
    width: 100%;
    background: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(10px);
    padding: 1rem 0;
    z-index: 1000;
    box-shadow: 0 2px 10px rgba(0,0,0,0.1);
}

.content {
    padding: 2rem 0;
    background: white;
    border-radius: 8px;
    margin: 2rem 0;
    box-shadow: 0 4px 20px rgba(0,0,0,0.1);
}

.footer {
    background: #333;
    color: white;
    text-align: center;
    padding: 2rem 0;
}

@media (max-width: 768px) {
    .hero h1 {
        font-size: 2.5rem;
    }
    .container {
        padding: 0 1rem;
    }
}
"#;
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/css")
                .header("cache-control", "public, max-age=31536000")
                .body(Full::new(Bytes::from(css_content)))
                .unwrap());
        }

        // JavaScript Resources
        "/js/app.js" => {
            let js_content = r#"
// Application JavaScript - Core functionality
console.log('103 Early Hints Demo - App JS Loaded');

document.addEventListener('DOMContentLoaded', function() {
    console.log('DOM loaded, initializing app...');
    
    // Simulate app initialization
    const loadTime = performance.now();
    console.log(`App initialized in ${loadTime.toFixed(2)}ms`);
    
    // Add interactive features
    const buttons = document.querySelectorAll('button');
    buttons.forEach(button => {
        button.addEventListener('click', function() {
            console.log('Button clicked:', this.textContent);
        });
    });
    
    // Performance monitoring
    if (window.PerformanceObserver) {
        const observer = new PerformanceObserver((list) => {
            list.getEntries().forEach((entry) => {
                if (entry.initiatorType === 'link' && entry.name.includes('103')) {
                    console.log('Early Hint resource loaded:', entry.name, `in ${entry.duration}ms`);
                }
            });
        });
        observer.observe({entryTypes: ['resource']});
    }
});
"#;
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/javascript")
                .header("cache-control", "public, max-age=31536000")
                .body(Full::new(Bytes::from(js_content)))
                .unwrap());
        }

        "/js/vendor.js" => {
            let js_content = r#"
// Vendor JavaScript - Third party libraries simulation
console.log('103 Early Hints Demo - Vendor JS Loaded');

// Simulate a small utility library
window.EarlyHintsDemo = {
    version: '1.0.0',
    
    formatTime: function(ms) {
        return `${ms.toFixed(2)}ms`;
    },
    
    measureResourceTiming: function() {
        const resources = performance.getEntriesByType('resource');
        const hintedResources = resources.filter(r => 
            r.name.includes('/css/') || 
            r.name.includes('/js/') || 
            r.name.includes('/fonts/') ||
            r.name.includes('/images/') ||
            r.name.includes('/api/')
        );
        
        console.group('103 Early Hints Resource Timing');
        hintedResources.forEach(resource => {
            console.log(`${resource.name}: ${this.formatTime(resource.duration)}`);
        });
        console.groupEnd();
        
        return hintedResources;
    },
    
    init: function() {
        console.log('Early Hints Demo Utils initialized');
        
        // Measure performance after page load
        window.addEventListener('load', () => {
            setTimeout(() => this.measureResourceTiming(), 1000);
        });
    }
};

// Auto-initialize
EarlyHintsDemo.init();
"#;
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/javascript")
                .header("cache-control", "public, max-age=31536000")
                .body(Full::new(Bytes::from(js_content)))
                .unwrap());
        }

        // Font Resources (simulated WOFF2)
        "/fonts/main.woff2" | "/fonts/icons.woff2" => {
            // In a real app, these would be actual font files
            // For demo purposes, return a small binary-like response
            let font_simulation = b"WOFF2\x00\x01\x00\x00\x00\x00\x02\x00"; // WOFF2 magic bytes + minimal data
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "font/woff2")
                .header("cache-control", "public, max-age=31536000")
                .header("access-control-allow-origin", "*")
                .body(Full::new(Bytes::from(&font_simulation[..])))
                .unwrap());
        }

        // Image Resource (simulated WebP)
        "/images/hero.webp" => {
            // Minimal WebP header for simulation
            let webp_simulation = b"RIFF\x1A\x00\x00\x00WEBPVP8 \x0E\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "image/webp")
                .header("cache-control", "public, max-age=31536000")
                .body(Full::new(Bytes::from(&webp_simulation[..])))
                .unwrap());
        }

        // API Resource
        "/api/initial-data.json" => {
            let json_data = r#"{
  "title": "103 Early Hints Demo",
  "version": "1.0.0",
  "performance": {
    "early_hints_enabled": true,
    "resources_hinted": 8
  },
  "resources": [
    {"type": "css", "url": "/css/critical.css", "priority": "high"},
    {"type": "css", "url": "/css/layout.css", "priority": "high"},
    {"type": "js", "url": "/js/app.js", "priority": "high"},
    {"type": "js", "url": "/js/vendor.js", "priority": "medium"},
    {"type": "font", "url": "/fonts/main.woff2", "priority": "medium"},
    {"type": "font", "url": "/fonts/icons.woff2", "priority": "low"},
    {"type": "image", "url": "/images/hero.webp", "priority": "medium"},
    {"type": "json", "url": "/api/initial-data.json", "priority": "low"}
  ],
  "timestamp": "2024-12-08T19:40:00Z"
}"#;
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("access-control-allow-origin", "*")
                .header("cache-control", "public, max-age=300")
                .body(Full::new(Bytes::from(json_data)))
                .unwrap());
        }

        // Root path - serve HTML page with all the hinted resources
        "/" => {
            // Send 103 Early Hints using the early_hints_pusher API
            if let Ok(mut pusher) = early_hints_pusher(&mut req) {
                println!("Sending 103 Early Hints (all critical resources)");

                let start_time = Instant::now();

                let hints = Response::builder()
                    .status(StatusCode::EARLY_HINTS)
                    // Critical CSS (highest priority - render blocking)
                    .header("link", "</css/critical.css>; rel=preload; as=style")
                    .header("link", "</css/layout.css>; rel=preload; as=style")
                    // Critical JavaScript (high priority - interaction)
                    .header("link", "</js/app.js>; rel=preload; as=script")
                    .header("link", "</js/vendor.js>; rel=preload; as=script")
                    // Fonts (medium priority - text rendering)
                    .header(
                        "link",
                        "</fonts/main.woff2>; rel=preload; as=font; crossorigin",
                    )
                    .header(
                        "link",
                        "</fonts/icons.woff2>; rel=preload; as=font; crossorigin",
                    )
                    // Hero image (medium priority - above fold)
                    .header("link", "</images/hero.webp>; rel=preload; as=image")
                    // Metadata for tracking
                    .header("x-resource-count", "7")
                    .header("x-priority-order", "css,js,fonts,images")
                    .body(())
                    .unwrap();

                if let Err(e) = pusher.send_hints(hints).await {
                    eprintln!("Failed to send hints: {}", e);
                } else {
                    let send_duration = start_time.elapsed();
                    println!("103 Early Hints sent in: {:?}", send_duration);
                    println!("   7 resources hinted in single response");
                    println!("   Browser processes once, starts all preloads immediately");
                }

                // Simulate realistic server processing time
                println!("Processing request (simulating database queries, template rendering...)");
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            let html_content = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>103 Early Hints Demo - Hyper HTTP/2 Server</title>
    
    <!-- Preloaded CSS files -->
    <link rel="stylesheet" href="/css/critical.css">
    <link rel="stylesheet" href="/css/layout.css">
    
    <!-- Preloaded font files -->
    <link rel="preload" href="/fonts/main.woff2" as="font" type="font/woff2" crossorigin>
    <link rel="preload" href="/fonts/icons.woff2" as="font" type="font/woff2" crossorigin>
</head>
<body>
    <nav class="navbar">
        <div class="container">
            <h2>103 Early Hints Demo</h2>
        </div>
    </nav>

    <section class="hero">
        <div class="container">
            <h1>HTTP/2 Early Hints</h1>
            <p>Demonstrating 103 Early Hints with Hyper</p>
            <button onclick="EarlyHintsDemo.measureResourceTiming()">Measure Performance</button>
        </div>
        <!-- Preloaded hero image -->
        <img src="/images/hero.webp" alt="Hero" style="display: none;" onload="console.log('Hero image loaded')">
    </section>

    <main class="content">
        <div class="container">
            <h2>Resource Loading Analysis</h2>
            <p>This page demonstrates 103 Early Hints by preloading 7 critical resources:</p>
            <ul>
                <li><strong>CSS:</strong> critical.css, layout.css</li>
                <li><strong>JavaScript:</strong> app.js, vendor.js</li>
                <li><strong>Fonts:</strong> main.woff2, icons.woff2</li>
                <li><strong>Images:</strong> hero.webp</li>
            </ul>
            
            <h3>Performance Benefits</h3>
            <p>With 103 Early Hints, the browser can start downloading critical resources 
               while the server is still processing the main request, reducing overall page load time.</p>
            
            <div id="api-data">Loading API data...</div>
        </div>
    </main>

    <footer class="footer">
        <div class="container">
            <p>&copy; 2024 Hyper HTTP/2 Early Hints Demo</p>
        </div>
    </footer>

    <!-- Preloaded JavaScript files -->
    <script src="/js/vendor.js"></script>
    <script src="/js/app.js"></script>
    
    <!-- Load API data -->
    <script>
        fetch('/api/initial-data.json')
            .then(response => response.json())
            .then(data => {
                document.getElementById('api-data').innerHTML = 
                    '<h4>API Data Loaded:</h4><pre>' + JSON.stringify(data, null, 2) + '</pre>';
            })
            .catch(error => {
                document.getElementById('api-data').innerHTML = 
                    '<p>Error loading API data: ' + error.message + '</p>';
            });
    </script>
</body>
</html>"#;

            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/html; charset=utf-8")
                .header("x-server", "hyper-103")
                .header("x-total-resources-hinted", "7")
                .body(Full::new(Bytes::from(html_content)))
                .unwrap());
        }

        // Default 404 handler
        _ => {
            let not_found_html = format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <title>404 Not Found</title>
    <style>
        body {{ font-family: Arial, sans-serif; text-align: center; padding: 2rem; }}
        .error {{ color: #e74c3c; }}
    </style>
</head>
<body>
    <h1 class="error">404 Not Found</h1>
    <p>The requested resource <code>{}</code> was not found.</p>
    <p><a href="/">← Back to Demo</a></p>
</body>
</html>"#,
                path
            );

            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("content-type", "text/html")
                .body(Full::new(Bytes::from(not_found_html)))
                .unwrap());
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    pretty_env_logger::init();

    let addr: SocketAddr = ([0, 0, 0, 0], 3000).into();

    // Load provided certificates or fallback to self-signed
    let (certs, key) = match load_certificates() {
        Ok((certs, key)) => {
            println!("Loaded certificates from /tmp/cert.txt and /tmp/key.txt");
            (certs, key)
        }
        Err(e) => {
            println!(
                "Failed to load provided certificates ({}), generating self-signed certificate...",
                e
            );
            generate_self_signed_cert()
        }
    };

    // Configure TLS
    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    // Enable HTTP/2
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let tls_acceptor = TlsAcceptor::from(std::sync::Arc::new(config));

    // Create TCP listener
    let listener = TcpListener::bind(addr).await?;
    println!("103 Early Hints Server listening on https://{}", addr);
    println!("Test: curl -k --http2 -v https://localhost:3000/");
    println!("Expected: 1 103 response + 1 final 200 response");
    println!("Benefits: Minimal browser overhead, maximum performance");

    loop {
        let (tcp_stream, _) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();

        tokio::spawn(async move {
            // Perform TLS handshake
            let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                Ok(stream) => stream,
                Err(e) => {
                    eprintln!("TLS handshake failed: {}", e);
                    return;
                }
            };

            // Serve HTTP/2 connection with Early Hints support enabled
            let service = service_fn(handle_request);

            if let Err(e) = http2::Builder::new(TokioExecutor)
                .enable_informational() // Enable 103 Early Hints support
                .serve_connection(TokioIo::new(tls_stream), service)
                .await
            {
                eprintln!("HTTP/2 connection error: {}", e);
            }
        });
    }
}
