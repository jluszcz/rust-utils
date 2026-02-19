use anyhow::{Context, Result};
use chrono::Utc;
use log::debug;
use std::env;
use std::future::Future;
use std::path::{Path, PathBuf};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

/// Returns a date-stamped cache path in the system temp directory.
///
/// The path has the form `$TMPDIR/<name>.YYYYMMDD.json`, where the date
/// is today's UTC date. A new path is returned each calendar day, which
/// naturally expires the previous day's cache.
pub fn dated_cache_path(name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!("{}.{}.json", name, Utc::now().date_naive().format("%Y%m%d")));
    path
}

/// Cache-aside helper: returns cached content if present, otherwise calls `query`,
/// writes the result to `cache_path`, and returns it.
///
/// When `use_cache` is `false` the cache is bypassed entirely — no read or write occurs.
pub async fn try_cached_query<F>(
    use_cache: bool,
    cache_path: &Path,
    query: impl Fn() -> F,
) -> Result<String>
where
    F: Future<Output = Result<String>>,
{
    match try_cached(use_cache, cache_path).await? {
        Some(cached) => Ok(cached),
        None => {
            let response = query().await?;
            try_write_cache(use_cache, cache_path, &response).await?;
            Ok(response)
        }
    }
}

async fn try_cached(use_cache: bool, cache_path: &Path) -> Result<Option<String>> {
    if use_cache && cache_path.exists() {
        debug!("Reading cache file: {cache_path:?}");
        Ok(Some(
            fs::read_to_string(cache_path)
                .await
                .with_context(|| format!("Failed to read cache file: {cache_path:?}"))?,
        ))
    } else {
        Ok(None)
    }
}

async fn try_write_cache(use_cache: bool, cache_path: &Path, response: &str) -> Result<()> {
    if use_cache {
        debug!("Writing response to cache file: {cache_path:?}");

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(cache_path)
            .await
            .with_context(|| format!("Failed to create or open cache file: {cache_path:?}"))?;

        file.write_all(response.as_bytes())
            .await
            .with_context(|| format!("Failed to write data to cache file: {cache_path:?}"))?;
    }

    Ok(())
}
