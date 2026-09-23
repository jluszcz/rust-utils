# AGENTS.md

This file provides guidance to AI coding agents when working with code in this repository.

## Development Commands

### Build and Test
- `cargo build` - Build the library
- `cargo test` - Run all tests
- `cargo fmt --check` - Check formatting (same as CI)
- `cargo clippy --all-targets --all-features -- -D warnings` - Run linter with warnings as errors

### Production Build

These are the commands CI runs. The `aarch64-unknown-linux-musl` target is **not** installed locally by default and
nothing in this repo (no `rust-toolchain.toml`, no `.cargo/config.toml`) declares it — run `rustup target add
aarch64-unknown-linux-musl` first if you need to reproduce a CI failure locally.

- `cargo build --target aarch64-unknown-linux-musl --all-features` - Build for ARM64 Linux (Lambda target)
- `cargo test --target aarch64-unknown-linux-musl --all-features` - Test on target platform
- `cargo clippy --target aarch64-unknown-linux-musl --all-targets --all-features -- -D warnings` - Lint for target platform

## Architecture

This is a Rust utilities library (`jluszcz_rust_utils`) designed for AWS Lambda functions. The codebase provides:

### Core Components
- **Logger setup** (`set_up_logger` in `lib.rs`) - Configures structured logging with timestamp formatting for Lambda environments; logs the rustc version once configured
- **Lambda initialization** (`lambda::init`) - Thin wrapper around `set_up_logger` that accepts `impl Into<Verbosity>`
- **Lambda entry point** (`lambda::run`, feature `lambda`) - Installs the TLS provider, sets up logging once at cold start, and serves the handler
- **HTTP + cache** (`query`, `cache`; feature `query`) - Shared client, retry with body-carrying errors, on-disk cache, typed JSON variants of both
- **AWS config** (`aws`, feature `aws`) - `SdkConfig` loading with a standard retry policy
- **Bedrock** (`bedrock`, feature `bedrock`) - Converse-API text generation; prompt and cleanup stay with the caller
- **CLI args** (`cli`, feature `cli`) - Flattenable clap verbosity argument
- **TLS** (`tls`, feature `tls`) - `rustls` crypto provider installation

Features are additive and default-off. Consumers track this crate as an unpinned git dependency, so
`main` must keep building for repos that haven't opted into a new feature yet.

### Key Dependencies
- `anyhow` - Error handling
- `fern` + `log` - Structured logging
- `chrono` - Timestamp formatting
- (`query`) `reqwest`, `backon`, `serde`, `serde_json`, `tokio` - HTTP with retry and file-based cache
- (`cli`) `clap` - Shared verbosity argument
- (`aws`, `bedrock`) `aws-config`, `aws-sdk-bedrockruntime` - AWS configuration and Bedrock
- (`tls`, `lambda`) `rustls`, `lambda_runtime` - TLS provider and Lambda runtime

### Build System
- Uses `build.rs` to capture rustc version at build time via `RUSTC_VERSION` environment variable
- CI builds against `aarch64-unknown-linux-musl` (ARM64 Lambda runtime); the target is selected by the CI workflow,
  not by anything checked into this repo

### Documentation
- `src/lib.rs` sets `#![warn(missing_docs)]`, and CI lints with `-D warnings`, so **every new public item needs a doc
  comment or the build fails**. This is deliberate: the crate is consumed by sibling repos whose authors read
  rustdoc rather than the source.
- Document the *why* a caller can't infer: `set_up_logger` caps dependencies at `Warn` so verbosity doesn't bury the
  application's own output; `lambda::init` is `async` and fallible for future headroom rather than present need.
  `cache.rs` and `query.rs` were already written this way and are the template.

### Dependency Versioning
- Pin 0.x dependencies to their **minor** version (`chrono = "0.4"`, not `chrono = "0"`). For 0.x crates the minor
  version is the breaking axis, so a bare `"0"` resolves to `<1.0.0` and lets breaking releases through silently.
- Sibling repos consume this crate as an unpinned git dependency, so a break here fans out to all of them.
- AWS SDK crates ship a `default` feature set that includes `rustls`, which is the legacy hyper-0.14 client
  stack (rustls 0.21 / rustls-webpki 0.101) — unmaintained and a standing source of Dependabot alerts. Take
  `default-features = false` and name `default-https-client` (plus `behavior-version-latest` and `rt-tokio`)
  explicitly when adding or bumping an `aws-sdk-*` dependency.

### Testing
- Unit tests live alongside the code, in a `mod tests` per module
- `query.rs` tests `send` against a `TcpListener` bound to an ephemeral port rather than mocking
  `reqwest`, which is what makes the retry *count* observable
- Prefer extracting a pure function over testing through an AWS or network client: `resolve_model_id`,
  `extract_text`, and `truncate_body` exist in that shape for this reason
- CI is a thin caller of `jluszcz/github-utils/.github/workflows/rust-ci.yml` (`.github/workflows/ci.yml`), which
  runs build, test, `cargo fmt --check`, and `cargo clippy -- -D warnings` on `ubuntu-24.04-arm` with `--all-features`.
  The steps live in that shared workflow, not in this repo.
