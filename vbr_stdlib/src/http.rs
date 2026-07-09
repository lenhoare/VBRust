//! Wraps `ureq` — simple, blocking HTTP. Each call is an independent one-shot
//! request; there is no shared connection or session here. For a reused client
//! (connection pool, cookies, auth across calls), drop to inline Rust or a `.rs`
//! module holding a `reqwest::Client` — VBR keeps the simple case simple and
//! sends the stateful case to the escape hatch.

use std::collections::HashMap;
use std::time::Duration;

/// Overall per-request timeout. Without one, a hung server means the call
/// never returns — in a UI the event would sit on "sending…" forever with no
/// error. Generous, because LLM endpoints legitimately take a while.
const TIMEOUT_SECS: u64 = 60;

pub struct Http;

impl Http {
    /// Fetch a URL with GET and return the response body. Times out (as an
    /// `Err`) after 60 seconds.
    pub fn get(url: &str) -> Result<String, String> {
        Self::get_with_timeout(url, TIMEOUT_SECS)
    }

    /// POST a string body to a URL with the given request headers, and return
    /// the response body. The headers map carries whatever the endpoint needs —
    /// `Content-Type`, an `Authorization: Bearer …` token, and so on; pass an
    /// empty map for none. Times out (as an `Err`) after 60 seconds.
    pub fn post(
        url: &str,
        body: &str,
        headers: HashMap<String, String>,
    ) -> Result<String, String> {
        Self::post_with_timeout(url, body, headers, TIMEOUT_SECS)
    }

    fn get_with_timeout(url: &str, secs: u64) -> Result<String, String> {
        ureq::get(url)
            .timeout(Duration::from_secs(secs))
            .call()
            .map_err(|e| e.to_string())?
            .into_string()
            .map_err(|e| e.to_string())
    }

    fn post_with_timeout(
        url: &str,
        body: &str,
        headers: HashMap<String, String>,
        secs: u64,
    ) -> Result<String, String> {
        let mut request = ureq::post(url).timeout(Duration::from_secs(secs));
        for (name, value) in &headers {
            request = request.set(name, value);
        }
        request
            .send_string(body)
            .map_err(|e| e.to_string())?
            .into_string()
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    /// Spin up a one-shot loopback HTTP server that replies with `body`, and
    /// return its `http://127.0.0.1:PORT/` URL. Keeps the test hermetic — no
    /// external network, fully deterministic, like the rest of the suite.
    fn serve_once(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf); // consume the request
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://127.0.0.1:{}/", port)
    }

    #[test]
    fn get_returns_body() {
        let url = serve_once("hello from the test server");
        assert_eq!(Http::get(&url).unwrap(), "hello from the test server");
    }

    #[test]
    fn post_returns_body() {
        let url = serve_once("posted ok");
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        assert_eq!(Http::post(&url, "payload", headers).unwrap(), "posted ok");
    }

    #[test]
    fn bad_host_is_an_err() {
        // Nothing is listening on this port → a transport error, surfaced as a String.
        assert!(Http::get("http://127.0.0.1:1/").is_err());
    }

    #[test]
    fn hung_server_times_out_as_err() {
        // A server that accepts the connection and then says nothing — the
        // shape of a hung endpoint. The timeout turns "waits forever" into an
        // Err. (Tested through the private helper with a 1-second timeout so
        // the test is fast; the public calls use the same path with 60.)
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                thread::sleep(std::time::Duration::from_secs(10));
            }
        });
        let url = format!("http://127.0.0.1:{}/", port);
        assert!(Http::get_with_timeout(&url, 1).is_err());
    }
}
