use again::RetryPolicy;
use anyhow::{Context, Result};
use log::trace;
use reqwest::{Client, Method};
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Returns a shared singleton [`reqwest::Client`] configured for API requests.
///
/// The client is initialized once with a 30s request timeout, 10s connect timeout,
/// 90s pool idle timeout, a per-host connection limit of 10, and gzip decompression.
pub fn http_client() -> Result<&'static Client> {
    if let Some(client) = HTTP_CLIENT.get() {
        return Ok(client);
    }
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

/// Performs an HTTP GET request with exponential-backoff retry.
///
/// Retries up to 3 times with 100ms base delay, 2s max delay, and jitter.
/// Sets `Accept: application/json` and `Accept-Encoding: gzip` headers,
/// and returns an error for non-2xx responses.
pub async fn http_get<T>(url: &str, params: &T) -> Result<String>
where
    T: Serialize + ?Sized,
{
    let client = http_client()?;

    let retry_policy = RetryPolicy::exponential(Duration::from_millis(100))
        .with_jitter(true)
        .with_max_delay(Duration::from_secs(2))
        .with_max_retries(3);

    let response = retry_policy
        .retry(|| {
            client
                .request(Method::GET, url)
                .header("Accept", "application/json")
                .header("Accept-Encoding", "gzip")
                .query(params)
                .send()
        })
        .await
        .with_context(|| format!("Failed to make HTTP request to {url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP request failed for {url}"))?
        .text()
        .await
        .with_context(|| "Failed to read response body")?;

    trace!("{response}");

    Ok(response)
}
