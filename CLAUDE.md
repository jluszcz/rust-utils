# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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

### Key Dependencies
- `anyhow` - Error handling
- `fern` + `log` - Structured logging
- `chrono` - Timestamp formatting
- (`query` feature) `reqwest`, `backon`, `serde`, `tokio` - HTTP GET with retry and file-based cache

### Build System
- Uses `build.rs` to capture rustc version at build time via `RUSTC_VERSION` environment variable
- CI builds against `aarch64-unknown-linux-musl` (ARM64 Lambda runtime); the target is selected by the CI workflow,
  not by anything checked into this repo

### Dependency Versioning
- Pin 0.x dependencies to their **minor** version (`chrono = "0.4"`, not `chrono = "0"`). For 0.x crates the minor
  version is the breaking axis, so a bare `"0"` resolves to `<1.0.0` and lets breaking releases through silently.
- Five sibling repos consume this crate as an unpinned git dependency, so a break here fans out to all of them.

### Testing
- Unit tests in `cache.rs` and `query.rs` modules
- CI is a thin caller of `jluszcz/github-utils/.github/workflows/rust-ci.yml@v1` (`.github/workflows/ci.yml`), which
  runs build, test, `cargo fmt --check`, and `cargo clippy -- -D warnings` on `ubuntu-24.04-arm` with `--all-features`.
  The steps live in that shared workflow, not in this repo.
