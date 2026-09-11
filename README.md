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

### Lambda entry point (`lambda::run`) — feature `lambda`

Replaces the `main` each Lambda binary writes by hand: installs the `rustls` crypto provider, sets up logging once at cold start, and serves the handler until the runtime shuts down. The handler is any `Fn(LambdaEvent<T>) -> Future<Output = Result<R, lambda_runtime::Error>>` with `T: DeserializeOwned` and `R: Serialize`.

```rust,ignore
#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    lambda::run(APP_NAME, module_path!(), false, function).await
}
```

### TLS provider (`tls::install_default_provider`) — features `tls`, `tls-ring`

Installs the process-wide `rustls` crypto provider. Without one, building the client panics at runtime rather than the build failing. `tls` installs `aws-lc-rs`; `tls-ring` installs `ring`, a smaller build that needs no CMake. `tls` wins when both are enabled, so a `lambda` consumer — which implies `tls` — can't get `ring` by also enabling `tls-ring`. Idempotent. `lambda::run` calls it (the `lambda` feature implies `tls`); every other binary calls it at the top of `main`, including one that takes `query` without `lambda`.

### HTTP client (`query::http_client`) — feature `query`

Returns a shared singleton `reqwest::Client` configured with:
- 30s request timeout, 10s connect timeout
- 90s pool idle timeout, max 10 idle connections per host
- gzip decompression enabled

The client pins no `rustls` crypto provider, so a consumer chooses one: enable `tls` or `tls-ring` and call `tls::install_default_provider` before the first call — `query::http_client` panics without one.

### Request with retry (`query::send`) — feature `query`

Sends any `reqwest::RequestBuilder`, retrying with exponential backoff where that is safe (up to 3 attempts, 100ms base delay, 2s max, with jitter). Retries cover transport errors and transient HTTP responses (5xx, 429); other non-2xx responses are returned immediately. Either way the error carries the response body (truncated to 1 KiB), which `reqwest`'s own `error_for_status` discards.

A request is sent exactly once when its method isn't idempotent (POST, PATCH) — a 5xx can arrive after the write committed, so retrying a create would duplicate it — or when its body can't be replayed.

### HTTP GET with retry (`query::http_get`) — feature `query`

`send` for a GET, setting `Accept: application/json` and `Accept-Encoding: gzip` headers and serializing query parameters. `query::http_get_json` adds deserialization into a caller-supplied type.

### File-based cache (`cache`) — feature `query`

Two helpers for a simple cache-aside pattern backed by the filesystem:

- **`dated_cache_path(name)`** — Returns a path in the system temp directory of the form `$TMPDIR/<name>.YYYYMMDD.json`. The date-stamped filename naturally expires the cache each calendar day.
- **`try_cached_query(mode, cache_path, query)`** — Returns cached content if the file exists; otherwise calls the async `query` closure, writes the result to `cache_path`, and returns it. Pass `CacheMode::Disabled` to bypass the cache entirely.
- **`try_cached_query_json(mode, cache_path, query)`** — The same, deserialized into a caller-supplied type. The cache still stores the raw text, so changing the type doesn't invalidate existing cache files.

### Verbosity flag (`cli::VerbosityArgs`) — feature `cli`

A flattenable clap argument providing the repeatable `-v` flag, converting into `Verbosity` (absent → Info, `-v` → Debug, `-vv` or more → Trace). Flatten it into an application's own `Parser` struct with `#[command(flatten)]` so every binary shares one spelling and one help string.

### AWS configuration (`aws::config`) — feature `aws`

Loads the ambient AWS configuration with `BehaviorVersion::latest()` and a standard retry policy (3 attempts; `aws::config_with_max_attempts` raises it for long batch work). Takes an optional region override, for CLIs that accept `--region`; pass `None` in a Lambda to accept the ambient one. The SDK applies no retries unless asked, which is the reason to route configuration through here.

### Bedrock text generation (`bedrock::BedrockClient`) — feature `bedrock`

Sends a single-turn prompt through the Bedrock Converse API and returns the model's reply verbatim. The model is `us.amazon.nova-2-lite-v1:0` unless `BEDROCK_MODEL_ID` overrides it.

Prompt construction and cleanup of the reply stay with the caller — those are the parts that differ per application. Construct with `from_env`, or `from_env_if_credentialed` to get `None` rather than per-call failures on a machine without AWS credentials. `generate_with_timeout` bounds the call for callers on a deadline of their own.

## Features

| Feature | Adds |
|---------|------|
| *(default)* | Logging, Lambda init |
| `query` | HTTP client, HTTP GET with retry, file-based cache |
| `cli` | Shared clap verbosity argument |
| `aws` | Shared AWS SDK configuration |
| `bedrock` | Bedrock Converse client (implies `aws`) |
| `tls` | `rustls` crypto provider installation (`aws-lc-rs`) |
| `tls-ring` | The same, on `ring` — a smaller build, no CMake |
| `lambda` | Lambda entry point (implies `tls`) |
