//! HTTP query helpers: a shared client, retries with exponential backoff, and
//! typed JSON responses.

use anyhow::{Context, Result, anyhow};
use backon::{ExponentialBuilder, Retryable};
use log::trace;
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::OnceLock;
use std::time::Duration;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// How much of an error response body to keep in the error message. Enough to
/// carry a typical API error payload, bounded so a server that returns an HTML
/// page doesn't dump it into the logs.
const MAX_ERROR_BODY_LEN: usize = 1024;

/// Returns a shared singleton [`reqwest::Client`] configured for API requests.
///
/// The client is initialized once with a 30s request timeout, 10s connect timeout,
/// 90s pool idle timeout, a per-host connection limit of 10, and gzip decompression.
///
/// **A `rustls` crypto provider must be installed before the first call.**
/// This crate's reqwest build pins none, so the application chooses. Enabling
/// `tls` or `tls-ring` is that choice, and this installs it on the first call;
/// a provider the application installed itself is left alone. With neither
/// feature the application must install one before calling this, or building
/// the client panics rather than the build failing.
pub fn http_client() -> Result<&'static Client> {
    if let Some(client) = HTTP_CLIENT.get() {
        return Ok(client);
    }

    // reqwest's `rustls-no-provider` build panics inside `build()` when no
    // provider is installed, which would step over the error this function
    // returns. Enabling a TLS feature is the application naming its provider,
    // so honor it here rather than leaving a panic where a `Result` belongs.
    #[cfg(any(feature = "tls", feature = "tls-ring"))]
    crate::tls::install_default_provider();

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(10)
        .gzip(true)
        .build()
        .context("Failed to create HTTP client")?;
    Ok(HTTP_CLIENT.get_or_init(|| client))
}

/// A failed attempt, tagged with whether retrying it could plausibly help.
///
/// `backon`'s retry predicate sees only the error, so the transient/permanent
/// decision has to be made where the status code is still in hand rather than
/// recovered from the error afterwards.
#[derive(Debug)]
enum QueryError {
    Transient(anyhow::Error),
    Permanent(anyhow::Error),
}

impl QueryError {
    fn is_transient(&self) -> bool {
        matches!(self, Self::Transient(_))
    }
}

impl From<QueryError> for anyhow::Error {
    fn from(value: QueryError) -> Self {
        match value {
            QueryError::Transient(e) | QueryError::Permanent(e) => e,
        }
    }
}

fn is_transient_status(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

fn truncate_body(mut body: String) -> String {
    if body.len() <= MAX_ERROR_BODY_LEN {
        return body;
    }

    // Walk back to a char boundary so multi-byte characters aren't split.
    let mut end = MAX_ERROR_BODY_LEN;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }

    body.truncate(end);
    body.push_str("... [truncated]");
    body
}

fn backoff() -> ExponentialBuilder {
    ExponentialBuilder::new()
        .with_min_delay(Duration::from_millis(100))
        .with_max_delay(Duration::from_secs(2))
        .with_max_times(3)
        .with_jitter()
}

/// Turns a non-2xx response into an error carrying the response body.
///
/// `reqwest`'s own `error_for_status` discards the body, which is where APIs
/// put the explanation of *why* the request was rejected.
async fn check_status(response: Response) -> Result<Response, QueryError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let url = response.url().clone();
    let body = truncate_body(response.text().await.unwrap_or_default());
    let error = anyhow!("HTTP {status} from {url}: {body}");

    if is_transient_status(status) {
        Err(QueryError::Transient(error))
    } else {
        Err(QueryError::Permanent(error))
    }
}

async fn send_once(request: RequestBuilder) -> Result<Response, QueryError> {
    // A transport error means the request never got a verdict, so it's always
    // worth another attempt.
    let response = request
        .send()
        .await
        .map_err(|e| QueryError::Transient(e.into()))?;

    check_status(response).await
}

/// Whether re-sending a request with this method is safe.
///
/// POST and PATCH are excluded: a 5xx can arrive after the server already
/// committed the write, so a retry would apply it twice.
fn is_idempotent(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::PUT | Method::DELETE | Method::OPTIONS | Method::TRACE
    )
}

/// Sends a request, retrying with exponential backoff when that is safe, using
/// the shared client's connection pool if the request was built from
/// [`http_client`].
///
/// Retries up to 3 times with 100ms base delay, 2s max delay, and jitter, and
/// cover transport errors and transient HTTP responses (5xx, 429). Other
/// non-2xx responses are returned immediately. Either way the error carries the
/// response body, truncated to a bounded length.
///
/// A request is sent exactly once, with no retry, when either:
///
/// - its method isn't idempotent (POST, PATCH), because a 5xx can arrive after
///   the write already committed — retrying a create would produce a duplicate;
/// - its body can't be replayed (a stream), because a retry would resume from a
///   partially consumed body.
///
/// A caller whose POST *is* safe to repeat — because the endpoint takes an
/// idempotency key, say — should drive [`http_client`] and this crate's retry
/// policy itself rather than reaching for a blanket opt-out here.
pub async fn send(request: RequestBuilder) -> Result<Response> {
    // Both conditions in one: a replayable body *and* a method that's safe to
    // repeat.
    let retryable = request
        .try_clone()
        .and_then(|attempt| attempt.build().ok())
        .is_some_and(|built| is_idempotent(built.method()));

    if !retryable {
        return Ok(send_once(request).await?);
    }

    let response = (|| async {
        let attempt = request
            .try_clone()
            .expect("cloneability was checked before the retry loop");
        send_once(attempt).await
    })
    .retry(backoff())
    .when(QueryError::is_transient)
    .await?;

    Ok(response)
}

/// Performs an HTTP GET request with exponential-backoff retry.
///
/// Sets `Accept: application/json` and `Accept-Encoding: gzip` headers and
/// serializes query parameters. See [`send`] for the retry and error behavior.
pub async fn http_get<T>(url: &str, params: &T) -> Result<String>
where
    T: Serialize + ?Sized,
{
    let client = http_client()?;

    // Parse here rather than letting `send` do it: reqwest's builder error
    // names the parse failure but not the URL that caused it, and this is the
    // last point at which we still have it. Errors from `send` already carry
    // the URL, so they need no further context.
    let parsed = reqwest::Url::parse(url).with_context(|| format!("Invalid request URL: {url}"))?;

    let request = client
        .request(Method::GET, parsed)
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip")
        .query(params);

    let response = send(request)
        .await?
        .text()
        .await
        .context("Failed to read response body")?;

    trace!("{response}");

    Ok(response)
}

/// [`http_get`] followed by JSON deserialization into `T`.
///
/// Use this when the raw body isn't needed; [`http_get`] remains the right call
/// when the response is cached as text or inspected before parsing.
pub async fn http_get_json<T, P>(url: &str, params: &P) -> Result<T>
where
    T: DeserializeOwned,
    P: Serialize + ?Sized,
{
    let body = http_get(url, params).await?;

    serde_json::from_str(&body).with_context(|| format!("Failed to parse JSON response from {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Installs a `rustls` crypto provider for the process, once.
    ///
    /// `http_client` documents that the application is responsible for this;
    /// here, the test suite is the application. reqwest's `rustls-no-provider`
    /// build panics when a `Client` is built with no provider installed, so
    /// this delegates to the crate's own feature-selected provider — falling
    /// back to `aws-lc-rs` directly when neither `tls` nor `tls-ring` is
    /// enabled — rather than hardcoding one, which would install the wrong
    /// provider under a `tls-ring`-only build and race `tls`'s own tests for
    /// the single process-wide slot.
    fn install_crypto_provider() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            #[cfg(any(feature = "tls", feature = "tls-ring"))]
            crate::tls::install_default_provider();
            #[cfg(not(any(feature = "tls", feature = "tls-ring")))]
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        });
    }

    /// Serves one canned response per connection, counting how many requests
    /// arrived — which is how the retry behavior is actually observable.
    fn serve(status_line: &str, body: &str) -> (String, Arc<AtomicUsize>) {
        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let url = format!("http://{}", listener.local_addr().expect("local addr"));

        let requests = Arc::new(AtomicUsize::new(0));
        let served = Arc::clone(&requests);

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                served.fetch_add(1, Ordering::SeqCst);
                let _ = stream.read(&mut [0u8; 1024]);
                let _ = stream.write_all(response.as_bytes());
            }
        });

        (url, requests)
    }

    #[tokio::test]
    async fn test_send_error_carries_response_body() {
        install_crypto_provider();
        let (url, requests) = serve("404 Not Found", "no such widget");

        let error = send(http_client().unwrap().get(&url))
            .await
            .expect_err("404 should be an error");

        let message = format!("{error}");
        assert!(message.contains("404"), "{message}");
        assert!(message.contains("no such widget"), "{message}");
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_send_does_not_retry_client_errors() {
        install_crypto_provider();
        let (url, requests) = serve("400 Bad Request", "malformed");

        let _ = send(http_client().unwrap().get(&url)).await;

        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_send_retries_server_errors() {
        install_crypto_provider();
        let (url, requests) = serve("503 Service Unavailable", "try later");

        let error = send(http_client().unwrap().get(&url))
            .await
            .expect_err("503 should exhaust retries and fail");

        assert!(format!("{error}").contains("503"));
        // The initial attempt plus three retries.
        assert_eq!(requests.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_send_does_not_retry_a_post() {
        install_crypto_provider();
        let (url, requests) = serve("503 Service Unavailable", "try later");

        let _ = send(http_client().unwrap().post(&url)).await;

        // A 5xx can arrive after the server committed the write, so retrying a
        // create would produce a duplicate.
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_send_does_not_retry_a_patch() {
        install_crypto_provider();
        let (url, requests) = serve("503 Service Unavailable", "try later");

        let _ = send(http_client().unwrap().patch(&url)).await;

        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_send_retries_idempotent_mutations() {
        install_crypto_provider();
        let (put_url, put_requests) = serve("503 Service Unavailable", "try later");
        let (delete_url, delete_requests) = serve("503 Service Unavailable", "try later");

        let _ = send(http_client().unwrap().put(&put_url)).await;
        let _ = send(http_client().unwrap().delete(&delete_url)).await;

        assert_eq!(put_requests.load(Ordering::SeqCst), 4);
        assert_eq!(delete_requests.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_send_returns_success_response() {
        install_crypto_provider();
        let (url, requests) = serve("200 OK", r#"{"ok":true}"#);

        let body = send(http_client().unwrap().get(&url))
            .await
            .expect("200 should succeed")
            .text()
            .await
            .expect("body");

        assert_eq!(body, r#"{"ok":true}"#);
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_http_get_names_the_url_only_once() {
        install_crypto_provider();
        let (url, _) = serve("404 Not Found", "gone");

        let error = http_get(&url, &[("a", "b")])
            .await
            .expect_err("404 should be an error");

        // `send` already names the URL; wrapping it again here produced
        // "HTTP request failed for <url>: HTTP 404 from <url>: ...".
        let message = format!("{error:#}");
        assert_eq!(message.matches("HTTP 404").count(), 1, "{message}");
        assert!(!message.contains("HTTP request failed for"), "{message}");
    }

    #[tokio::test]
    async fn test_http_get_reports_an_unusable_url() {
        install_crypto_provider();
        let error = http_get("not a url", &[("a", "b")])
            .await
            .expect_err("a malformed URL should be an error");

        // reqwest's own builder error says only "relative URL without a base",
        // which doesn't say *which* URL was wrong.
        let message = format!("{error:#}");
        assert!(message.contains("not a url"), "{message}");
    }

    #[test]
    fn test_http_client_returns_ok() {
        install_crypto_provider();
        assert!(http_client().is_ok());
    }

    #[test]
    fn test_http_client_is_singleton() {
        install_crypto_provider();
        let a = http_client().unwrap() as *const Client;
        let b = http_client().unwrap() as *const Client;
        assert_eq!(a, b);
    }

    #[test]
    fn test_idempotent_methods_are_retryable() {
        for method in [
            Method::GET,
            Method::HEAD,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::TRACE,
        ] {
            assert!(is_idempotent(&method), "{method} should be retryable");
        }
    }

    #[test]
    fn test_write_methods_are_not_retryable() {
        for method in [Method::POST, Method::PATCH] {
            assert!(!is_idempotent(&method), "{method} should not be retryable");
        }
    }

    #[test]
    fn test_server_errors_are_transient() {
        assert!(is_transient_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_transient_status(StatusCode::BAD_GATEWAY));
        assert!(is_transient_status(StatusCode::SERVICE_UNAVAILABLE));
    }

    #[test]
    fn test_too_many_requests_is_transient() {
        assert!(is_transient_status(StatusCode::TOO_MANY_REQUESTS));
    }

    #[test]
    fn test_client_errors_are_permanent() {
        assert!(!is_transient_status(StatusCode::NOT_FOUND));
        assert!(!is_transient_status(StatusCode::BAD_REQUEST));
        assert!(!is_transient_status(StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn test_short_body_is_not_truncated() {
        assert_eq!(truncate_body("nope".to_string()), "nope");
    }

    #[test]
    fn test_long_body_is_truncated() {
        let truncated = truncate_body("x".repeat(MAX_ERROR_BODY_LEN * 2));

        assert!(truncated.starts_with(&"x".repeat(MAX_ERROR_BODY_LEN)));
        assert!(truncated.ends_with("[truncated]"));
    }

    #[test]
    fn test_truncation_respects_char_boundaries() {
        // A multi-byte character straddling the limit must not be split, which
        // would panic on a naive byte slice.
        let body = "é".repeat(MAX_ERROR_BODY_LEN);

        let truncated = truncate_body(body);

        assert!(truncated.ends_with("[truncated]"));
    }
}
