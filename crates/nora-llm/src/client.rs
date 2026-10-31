use std::{
    fmt,
    sync::{
        Mutex, TryLockError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use reqwest::{
    Client,
    header::{ACCEPT, AUTHORIZATION, CONTENT_ENCODING, CONTENT_TYPE, HeaderValue, RETRY_AFTER},
};
use tokio::runtime::{Builder, Runtime};
use zeroize::Zeroizing;

use crate::{
    ApiConfig, ApiError, ConfigSummary, MAX_RESPONSE_BYTES, Result, TurnRequest, TurnResponse,
    types::{parse_response, prepare},
};

/// A single bounded in-flight turn per instance. Use on a dedicated blocking
/// worker thread, not from inside an async runtime or the GUI event loop.
/// Separate logical agents can share the instance serially; their usage still
/// belongs to the same caller-owned goal budget.
pub struct ResponsesClient {
    config: ApiConfig,
    client: Client,
    runtime: Mutex<Option<Runtime>>,
}

impl fmt::Debug for ResponsesClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponsesClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Drop for ResponsesClient {
    fn drop(&mut self) {
        let slot = match self.runtime.get_mut() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(runtime) = slot.take() {
            // OS resolver work started by Tokio may not be interruptible. Do
            // not hang application shutdown while waiting for such a thread.
            // This also avoids Tokio's blocking-drop panic in an async caller.
            runtime.shutdown_background();
        }
    }
}

impl ResponsesClient {
    pub fn new(config: ApiConfig) -> Result<Self> {
        if config.endpoint.scheme() != "https" {
            return Err(ApiError::Config("production transport requires HTTPS"));
        }
        Self::build(config, true)
    }

    fn build(config: ApiConfig, https_only: bool) -> Result<Self> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(ApiError::Protocol(
                "synchronous client construction requires a dedicated worker thread",
            ));
        }
        let runtime = Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(2)
            .thread_keep_alive(Duration::from_secs(5))
            .build()
            .map_err(|_| ApiError::Runtime)?;
        let client = Client::builder()
            .https_only(https_only)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .referer(false)
            .connection_verbose(false)
            .tls_sslkeylogfile(false)
            .tls_version_min(reqwest::tls::Version::TLS_1_2)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .connect_timeout(config.timeout.min(Duration::from_secs(15)))
            .read_timeout(config.timeout)
            .timeout(config.timeout)
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(1)
            .http1_max_headers(64)
            .http2_max_header_list_size(16_384)
            .user_agent(concat!("nora/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| ApiError::Transport)?;
        Ok(Self {
            config,
            client,
            runtime: Mutex::new(Some(runtime)),
        })
    }

    pub fn summary(&self) -> ConfigSummary {
        self.config.summary()
    }

    /// Trusted caller preflight for text that may be displayed/persisted before
    /// a request is built. Returns no credential bytes and performs no network.
    /// This detects only the exact configured key, not general sensitive data.
    pub fn validate_disclosure_text(&self, text: &str) -> Result<()> {
        if text.contains(self.config.key()) {
            Err(ApiError::CredentialDisclosure)
        } else {
            Ok(())
        }
    }

    /// Cancel is polled every20ms while awaiting headers/body; dropping the
    /// HTTP future stops local work but does not undo already billed server work.
    /// No network, provider error or timeout automatically retries a request.
    pub fn respond(&self, request: &TurnRequest, cancel: &AtomicBool) -> Result<TurnResponse> {
        if cancel.load(Ordering::Acquire) {
            return Err(ApiError::Cancelled);
        }
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(ApiError::Protocol(
                "synchronous facade requires a dedicated worker thread",
            ));
        }
        let prepared = prepare(&self.config, request)?;
        let encoded = std::str::from_utf8(&prepared.body)
            .map_err(|_| ApiError::Protocol("encoded request is not UTF-8"))?;
        if encoded.contains(self.config.key()) {
            return Err(ApiError::CredentialDisclosure);
        }
        let runtime_slot = self.runtime.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => ApiError::Busy,
            TryLockError::Poisoned(_) => ApiError::Runtime,
        })?;
        let runtime = runtime_slot.as_ref().ok_or(ApiError::Runtime)?;
        let key = Zeroizing::new(format!("Bearer {}", self.config.key()));
        let mut authorization =
            HeaderValue::from_str(&key).map_err(|_| ApiError::Config("invalid API key header"))?;
        authorization.set_sensitive(true);
        // The only URL accepted here was fixed and validated at construction.
        // There is no caller-controlled per-turn URL or redirect follow-up.
        let network = async {
            let mut response = self
                .client
                .post(self.config.endpoint.clone())
                .header(AUTHORIZATION, authorization)
                .header(CONTENT_TYPE, "application/json")
                .header(ACCEPT, "application/json")
                .body(prepared.body.clone())
                .send()
                .await
                .map_err(classify_transport)?;
            let status = response.status();
            if !status.is_success() {
                let retry_after_seconds = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|n| n.min(3600));
                // Never read/format an error body: providers/proxies can echo
                // prompts, credentials, private document text or HTML into it.
                return Err(ApiError::Http {
                    status: status.as_u16(),
                    retry_after_seconds,
                });
            }
            let content_type = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !content_type
                .split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
            {
                return Err(ApiError::Protocol("expected application/json response"));
            }
            if response
                .headers()
                .get(CONTENT_ENCODING)
                .is_some_and(|v| v != "identity")
            {
                return Err(ApiError::Protocol("compressed responses are not enabled"));
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
            {
                return Err(ApiError::Limit("response body bytes"));
            }
            let mut body = Zeroizing::new(Vec::with_capacity(16_384));
            while let Some(chunk) = response.chunk().await.map_err(classify_transport)? {
                if body
                    .len()
                    .checked_add(chunk.len())
                    .is_none_or(|n| n > MAX_RESPONSE_BYTES)
                {
                    return Err(ApiError::Limit("response body bytes"));
                }
                body.extend_from_slice(&chunk);
            }
            if cancel.load(Ordering::Acquire) {
                return Err(ApiError::Cancelled);
            }
            let result = parse_response(&body, &self.config.model, &prepared)?;
            reject_reflected_credential(&result, self.config.key())?;
            Ok(result)
        };
        runtime.block_on(async {
            tokio::select! {
                biased;
                _ = cancellation(cancel) => Err(ApiError::Cancelled),
                result = tokio::time::timeout(self.config.timeout, network) => {
                    result.map_err(|_| ApiError::Timeout)?
                }
            }
        })
    }
}

async fn cancellation(cancel: &AtomicBool) {
    loop {
        if cancel.load(Ordering::Acquire) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn classify_transport(error: reqwest::Error) -> ApiError {
    if error.is_timeout() {
        ApiError::Timeout
    } else {
        ApiError::Transport
    }
}

/// Even a successful upstream body must not turn an Authorization reflection
/// into UI text, a tool argument or persisted continuation state. Reject rather
/// than editing a tool's meaning. Encoded/derived secrets are not covered; keys
/// must never be placed in model input in the first place.
fn reject_reflected_credential(response: &TurnResponse, secret: &str) -> Result<()> {
    if response.text.contains(secret) {
        return Err(ApiError::Protocol("provider echoed a credential"));
    }
    let mut pending: Vec<_> = response
        .output_items
        .iter()
        .chain(response.tool_calls.iter().map(|c| &c.arguments))
        .collect();
    let mut nodes = 0usize;
    while let Some(value) = pending.pop() {
        nodes += 1;
        if nodes > 65_536 {
            return Err(ApiError::Limit("response JSON node budget"));
        }
        match value {
            serde_json::Value::String(text) if text.contains(secret) => {
                return Err(ApiError::Protocol("provider echoed a credential"));
            }
            serde_json::Value::Array(array) => {
                if pending.len() + array.len() > 65_536 {
                    return Err(ApiError::Limit("response JSON node budget"));
                }
                pending.extend(array);
            }
            serde_json::Value::Object(object) => {
                if pending.len() + object.len() > 65_536 {
                    return Err(ApiError::Limit("response JSON node budget"));
                }
                if object.keys().any(|key| key.contains(secret)) {
                    return Err(ApiError::Protocol("provider echoed a credential"));
                }
                pending.extend(object.values());
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{SocketAddr, TcpListener, TcpStream},
        sync::{Arc, mpsc},
        thread,
        time::Instant,
    };

    use super::*;
    use serde_json::{Value, json};

    fn config(address: SocketAddr, timeout: u64) -> ApiConfig {
        let text = format!(
            "NORA_LLM_API_KEY=synthetic-test-key\nNORA_LLM_MODEL=gpt-6-luna\nNORA_LLM_BASE_URL=http://{address}/v1\nNORA_LLM_TIMEOUT_SECONDS={timeout}"
        );
        ApiConfig::parse(text.as_bytes(), true).unwrap()
    }

    fn client(address: SocketAddr, timeout: u64) -> ResponsesClient {
        // The HTTP bypass exists only in this cfg(test) module and only binds
        // loopback. Neither config loading nor the public constructor permits it.
        ResponsesClient::build(config(address, timeout), false).unwrap()
    }

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut all = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let n = stream.read(&mut buffer).unwrap();
            assert_ne!(n, 0);
            all.extend_from_slice(&buffer[..n]);
            if let Some(header_end) = all.windows(4).position(|x| x == b"\r\n\r\n") {
                let headers = std::str::from_utf8(&all[..header_end]).unwrap();
                let length = headers
                    .lines()
                    .find_map(|s| {
                        s.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                if all.len() >= header_end + 4 + length {
                    return all;
                }
            }
            assert!(all.len() < 2_000_000);
        }
    }

    fn answer() -> String {
        json!({"id":"resp_test","model":"gpt-6-luna","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Nora sẵn sàng.","annotations":[]}]}],"usage":{"input_tokens":12,"output_tokens":9,"total_tokens":21}}).to_string()
    }

    fn server(response: String) -> (SocketAddr, mpsc::Receiver<Vec<u8>>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            tx.send(request).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        });
        (address, rx, handle)
    }

    fn ok(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn real_http_request_has_exact_route_auth_model_and_nonstorage() {
        let (address, rx, server) = server(ok(&answer()));
        let client = client(address, 3);
        let request = TurnRequest::user("Answer in Vietnamese.", "Xin chào");
        let response = client.respond(&request, &AtomicBool::new(false)).unwrap();
        assert_eq!(response.text, "Nora sẵn sàng.");
        let request = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        server.join().unwrap();
        let header_end = request.windows(4).position(|x| x == b"\r\n\r\n").unwrap();
        let headers = std::str::from_utf8(&request[..header_end])
            .unwrap()
            .to_ascii_lowercase();
        assert!(headers.starts_with("post /v1/responses http/1.1"));
        assert!(headers.contains("authorization: bearer synthetic-test-key"));
        let body: Value = serde_json::from_slice(&request[header_end + 4..]).unwrap();
        assert_eq!(body["model"], "gpt-6-luna");
        assert_eq!(body["reasoning"]["effort"], "xhigh");
        assert_eq!(body["store"], false);
        assert_eq!(body["stream"], false);
        assert!(!format!("{client:?}").contains("synthetic-test-key"));
    }

    #[test]
    fn errors_never_include_provider_body_and_rate_limits_do_not_retry() {
        let response="HTTP/1.1 429 Too Many Requests\r\nRetry-After: 5\r\nContent-Length: 37\r\nConnection: close\r\n\r\nsecret synthetic-test-key private-data".to_string();
        let (address, rx, server) = server(response);
        let client = client(address, 3);
        let error = client
            .respond(
                &TurnRequest::user("Answer briefly.", "hello"),
                &AtomicBool::new(false),
            )
            .unwrap_err();
        assert_eq!(
            error,
            ApiError::Http {
                status: 429,
                retry_after_seconds: Some(5)
            }
        );
        assert!(!format!("{error:?} {error}").contains("synthetic-test-key"));
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn successful_body_cannot_reflect_authorization_into_user_text() {
        let body = answer().replace("Nora sẵn sàng.", "synthetic-test-key");
        let (address, rx, server) = server(ok(&body));
        let client = client(address, 3);
        let error = client
            .respond(
                &TurnRequest::user("Answer briefly.", "hello"),
                &AtomicBool::new(false),
            )
            .unwrap_err();
        assert_eq!(error, ApiError::Protocol("provider echoed a credential"));
        assert!(!format!("{error:?} {error}").contains("synthetic-test-key"));
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn outgoing_request_containing_configured_credential_never_connects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let client = client(listener.local_addr().unwrap(), 3);
        let request = TurnRequest::user(
            "Answer briefly.",
            "A renamed note accidentally contains synthetic-test-key.",
        );
        let error = client
            .respond(&request, &AtomicBool::new(false))
            .unwrap_err();
        assert_eq!(error, ApiError::CredentialDisclosure);
        assert!(!format!("{error:?} {error}").contains("synthetic-test-key"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn redirects_are_not_followed_or_given_the_authorization_header() {
        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let response = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            target.local_addr().unwrap()
        );
        let (address, rx, server) = server(response);
        let client = client(address, 3);
        assert!(matches!(
            client.respond(
                &TurnRequest::user("Answer briefly.", "hello"),
                &AtomicBool::new(false)
            ),
            Err(ApiError::Http { status: 307, .. })
        ));
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        server.join().unwrap();
        assert_eq!(
            target.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn malformed_and_excessive_body_are_rejected_without_returning_contents() {
        for response in [
            ok("not JSON, secret=synthetic-test-key"),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_RESPONSE_BYTES + 1
            ),
        ] {
            let (address, rx, server) = server(response);
            let client = client(address, 3);
            let error = client
                .respond(
                    &TurnRequest::user("Answer briefly.", "hello"),
                    &AtomicBool::new(false),
                )
                .unwrap_err();
            assert!(matches!(error, ApiError::Protocol(_) | ApiError::Limit(_)));
            assert!(!format!("{error:?} {error}").contains("synthetic-test-key"));
            rx.recv_timeout(Duration::from_secs(3)).unwrap();
            server.join().unwrap();
        }
    }

    #[test]
    fn unknown_length_chunked_response_still_has_a_hard_body_bound() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
            let chunk = [b'x'; 16_384];
            for _ in 0..=(MAX_RESPONSE_BYTES / chunk.len()) {
                if stream
                    .write_all(b"4000\r\n")
                    .and_then(|_| stream.write_all(&chunk))
                    .and_then(|_| stream.write_all(b"\r\n"))
                    .is_err()
                {
                    return;
                }
            }
            let _ = stream.write_all(b"0\r\n\r\n");
        });
        let client = client(address, 3);
        assert_eq!(
            client
                .respond(
                    &TurnRequest::user("Answer briefly.", "hello"),
                    &AtomicBool::new(false)
                )
                .unwrap_err(),
            ApiError::Limit("response body bytes")
        );
        server.join().unwrap();
    }

    #[test]
    fn cancellation_interrupts_waiting_for_headers_and_client_can_serve_again() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            tx.send(()).unwrap();
            let mut byte = [0u8; 1];
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let _ = stream.read(&mut byte);
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            stream.write_all(ok(&answer()).as_bytes()).unwrap();
        });
        let client = Arc::new(client(address, 3));
        let worker_client = client.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let worker = thread::spawn(move || {
            worker_client.respond(
                &TurnRequest::user("Answer briefly.", "hello"),
                &worker_cancel,
            )
        });
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let start = Instant::now();
        cancel.store(true, Ordering::Release);
        assert_eq!(worker.join().unwrap().unwrap_err(), ApiError::Cancelled);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(
            client
                .respond(
                    &TurnRequest::user("Answer briefly.", "hello again"),
                    &AtomicBool::new(false)
                )
                .unwrap()
                .text,
            "Nora sẵn sàng."
        );
        server.join().unwrap();
    }

    #[test]
    fn deadline_and_pre_cancel_are_explicit() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            let mut b = [0u8; 1];
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let _ = stream.read(&mut b);
        });
        let client = client(address, 1);
        let request = TurnRequest::user("Answer briefly.", "hello");
        assert_eq!(
            client
                .respond(&request, &AtomicBool::new(true))
                .unwrap_err(),
            ApiError::Cancelled
        );
        assert_eq!(
            client
                .respond(&request, &AtomicBool::new(false))
                .unwrap_err(),
            ApiError::Timeout
        );
        server.join().unwrap();
    }
}
