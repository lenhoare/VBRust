//! Wraps `ureq` — simple, blocking HTTP. Each call is an independent one-shot
//! request; there is no shared connection or session here. For a reused client
//! (connection pool, cookies, auth across calls), drop to inline Rust or a `.rs`
//! module holding a `reqwest::Client` — VBR keeps the simple case simple and
//! sends the stateful case to the escape hatch.

use std::collections::HashMap;

pub struct Http;

impl Http {
    /// Fetch a URL with GET and return the response body.
    pub fn get(url: &str) -> Result<String, String> {
        ureq::get(url)
            .call()
            .map_err(|e| e.to_string())?
            .into_string()
            .map_err(|e| e.to_string())
    }

    /// POST a string body to a URL with the given request headers, and return
    /// the response body. The headers map carries whatever the endpoint needs —
    /// `Content-Type`, an `Authorization: Bearer …` token, and so on; pass an
    /// empty map for none.
    pub fn post(
        url: &str,
        body: &str,
        headers: HashMap<String, String>,
    ) -> Result<String, String> {
        let mut request = ureq::post(url);
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
}
