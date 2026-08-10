# rust-utils

Common utilities for Rust Lambdas

## Status

[![Status Badge](https://github.com/jluszcz/rust-utils/actions/workflows/ci.yml/badge.svg)](https://github.com/jluszcz/rust-utils/actions/workflows/ci.yml)

## Utilities

### Logging (`set_up_logger`)

Configures structured logging via `fern` with UTC timestamps in the format `YYYY-MM-DDTHH:MM:SS.mmmZ`. Accepts `impl Into<Verbosity>` (Info / Debug / Trace) and applies it to the app and calling module, while keeping other crates at `Warn`. Logs the rustc version (captured at build time via `build.rs`) at `info` once configured.

`Verbosity` converts from `bool` (false → Info, true → Debug) or `u8` (0 → Info, 1 → Debug, 2+ → Trace).

### Lambda initialization (`lambda::init`)

Thin entry-point helper that calls `set_up_logger`. Accepts `impl Into<Verbosity>`, so callers can pass a `bool`, a `u8`, or a `Verbosity` directly.

### HTTP client (`query::http_client`) — feature `query`

Returns a shared singleton `reqwest::Client` configured with:
- 30s request timeout, 10s connect timeout
- 90s pool idle timeout, max 10 idle connections per host
- gzip decompression enabled

### Request with retry (`query::send`) — feature `query`

Sends any `reqwest::RequestBuilder` with exponential-backoff retry (up to 3 attempts, 100ms base delay, 2s max, with jitter). Retries cover transport errors and transient HTTP responses (5xx, 429); other non-2xx responses are returned immediately. Either way the error carries the response body (truncated to 1 KiB), which `reqwest`'s own `error_for_status` discards. Requests whose body can't be replayed are sent exactly once.

### HTTP GET with retry (`query::http_get`) — feature `query`

`send` for a GET, setting `Accept: application/json` and `Accept-Encoding: gzip` headers and serializing query parameters. `query::http_get_json` adds deserialization into a caller-supplied type.

### File-based cache (`cache`) — feature `query`

Two helpers for a simple cache-aside pattern backed by the filesystem:

- **`dated_cache_path(name)`** — Returns a path in the system temp directory of the form `$TMPDIR/<name>.YYYYMMDD.json`. The date-stamped filename naturally expires the cache each calendar day.
- **`try_cached_query(mode, cache_path, query)`** — Returns cached content if the file exists; otherwise calls the async `query` closure, writes the result to `cache_path`, and returns it. Pass `CacheMode::Disabled` to bypass the cache entirely.
- **`try_cached_query_json(mode, cache_path, query)`** — The same, deserialized into a caller-supplied type. The cache still stores the raw text, so changing the type doesn't invalidate existing cache files.

### Verbosity flag (`cli::VerbosityArgs`) — feature `cli`

A flattenable clap argument providing the repeatable `-v` flag, converting into `Verbosity` (absent → Info, `-v` → Debug, `-vv` or more → Trace). Flatten it into an application's own `Parser` struct with `#[command(flatten)]` so every binary shares one spelling and one help string.

## Features

| Feature | Adds |
|---------|------|
| *(default)* | Logging, Lambda init |
| `query` | HTTP client, HTTP GET with retry, file-based cache |
| `cli` | Shared clap verbosity argument |
