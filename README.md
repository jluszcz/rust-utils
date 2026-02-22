# rust-utils

Common utilities for Rust Lambdas

## Status

[![Status Badge](https://github.com/jluszcz/rust-utils/actions/workflows/build-and-test.yml/badge.svg)](https://github.com/jluszcz/rust-utils/actions/workflows/build-and-test.yml)

## Utilities

### Logging (`set_up_logger`)

Configures structured logging via `fern` with UTC timestamps in the format `YYYY-MM-DDTHH:MM:SS.mmmZ`. Accepts a `Verbosity` level (Info / Debug / Trace) and applies it to the app and calling module, while keeping other crates at `Warn`.

`Verbosity` converts from `bool` (false → Info, true → Debug) or `u8` (0 → Info, 1 → Debug, 2+ → Trace).

### Lambda initialization (`lambda::init`)

Async entry-point helper that calls `set_up_logger` and then emits a `RustcVersion` CloudWatch metric for the running binary. The rustc version is captured at build time via `build.rs`.

### HTTP client (`query::http_client`) — feature `query`

Returns a shared singleton `reqwest::Client` configured with:
- 30s request timeout, 10s connect timeout
- 90s pool idle timeout, max 10 idle connections per host
- gzip decompression enabled

### HTTP GET with retry (`query::http_get`) — feature `query`

Performs an authenticated HTTP GET with exponential-backoff retry (up to 3 attempts, 100ms base delay, 2s max, with jitter). Sets `Accept: application/json` and `Accept-Encoding: gzip` headers, serializes query parameters, and returns an error for non-2xx responses.

### File-based cache (`cache`) — feature `query`

Two helpers for a simple cache-aside pattern backed by the filesystem:

- **`dated_cache_path(name)`** — Returns a path in the system temp directory of the form `$TMPDIR/<name>.YYYYMMDD.json`. The date-stamped filename naturally expires the cache each calendar day.
- **`try_cached_query(use_cache, cache_path, query)`** — Returns cached content if the file exists; otherwise calls the async `query` closure, writes the result to `cache_path`, and returns it. Set `use_cache = false` to bypass the cache entirely.

## Features

| Feature | Adds |
|---------|------|
| *(default)* | Logging, Lambda init, CloudWatch metrics |
| `query` | HTTP client, HTTP GET with retry, file-based cache |
