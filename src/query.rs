use again::RetryPolicy;
use anyhow::{Context, Result};
use log::trace;
use reqwest::{Client, Method};
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .gzip(true)
            .build()
            .expect("Failed to create HTTP client")
    })
}

pub async fn http_get<T>(url: &str, params: &T) -> Result<String>
where
    T: Serialize + ?Sized,
{
    let retry_policy = RetryPolicy::exponential(Duration::from_millis(100))
        .with_jitter(true)
        .with_max_delay(Duration::from_secs(2))
        .with_max_retries(3);

    let response = retry_policy
        .retry(|| {
            http_client()
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
