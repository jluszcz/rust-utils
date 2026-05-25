# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build and Test
- `cargo build` - Build the library
- `cargo test` - Run all tests
- `cargo clippy -- -D warnings` - Run linter with warnings as errors

### Production Build
- `cargo build --target aarch64-unknown-linux-musl` - Build for ARM64 Linux (Lambda target)
- `cargo test --target aarch64-unknown-linux-musl` - Test on target platform
- `cargo clippy --target aarch64-unknown-linux-musl -- -D warnings` - Lint for target platform

## Architecture

This is a Rust utilities library (`jluszcz_rust_utils`) designed for AWS Lambda functions. The codebase provides:

### Core Components
- **Logger setup** (`set_up_logger` in `lib.rs`) - Configures structured logging with timestamp formatting for Lambda environments; logs the rustc version once configured
- **Lambda initialization** (`lambda::init`) - Thin wrapper around `set_up_logger` that accepts `impl Into<Verbosity>`

### Key Dependencies
- `anyhow` - Error handling
- `fern` + `log` - Structured logging
- `chrono` - Timestamp formatting
- (`query` feature) `reqwest`, `backon`, `serde`, `tokio` - HTTP GET with retry and file-based cache

### Build System
- Uses `build.rs` to capture rustc version at build time via `RUSTC_VERSION` environment variable
- Configured for `aarch64-unknown-linux-musl` target (ARM64 Lambda runtime)

### Testing
- Unit tests in `cache.rs` and `query.rs` modules
- CI runs on ubuntu-24.04-arm with full build/test/lint pipeline
