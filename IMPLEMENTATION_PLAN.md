# Implementation Plan: Expand shared utilities

Extract code duplicated across the consuming repos into this crate. Every change is **additive** —
consumers track this crate as an unpinned git dependency, so `main` must keep building for them
untouched until they opt into the new features.

New features, all off by default:

| Feature   | Adds                                                          |
|-----------|---------------------------------------------------------------|
| `cli`     | `VerbosityArgs`, a flattenable clap `-v` argument             |
| `aws`     | `aws::config`, shared `SdkConfig` loading with retry defaults |
| `bedrock` | `bedrock::BedrockClient`, Converse-API text generation        |
| `lambda`  | `lambda::run`, entry-point wrapper (logging + rustls + serve) |

## Stage 1: Docs + `cli` feature

**Goal**: Drop the consumer-count specificity from the docs; ship `VerbosityArgs` behind `cli`.
**Success Criteria**: `cargo test --features cli` passes; `cargo build` (no features) unchanged.
**Tests**: `-v` → Debug, `-vv` → Trace, absent → Info, via clap parsing of a struct that flattens it.
**Status**: Complete

## Stage 2: `query` error handling + typed JSON

**Goal**: Non-2xx errors carry the response body; add `query::send` for arbitrary methods with the
shared client and retry policy; add `http_get_json` and `try_cached_query_json`.
**Success Criteria**: `cargo test --features query` passes; `http_get` behavior unchanged except for
richer error text.
**Tests**: transient/permanent classification by status; typed JSON round-trip through a cache hit;
`send` retry path for a body that can't be cloned.
**Status**: Not Started

## Stage 3: `aws` feature

**Goal**: `aws::config(region)` returning an `SdkConfig` with a standard retry policy, plus
`aws::has_credentials` for the probe mbtalerts does today.
**Success Criteria**: `cargo test --features aws` passes.
**Tests**: region override is applied; default (no region) resolves from the environment.
**Status**: Not Started

## Stage 4: `bedrock` feature

**Goal**: `BedrockClient` with `from_env`, `from_env_if_credentialed`, `generate`, and
`generate_with_timeout`. Prompt and post-processing stay with the caller.
**Success Criteria**: `cargo test --features bedrock` passes.
**Tests**: model id resolution (`BEDROCK_MODEL_ID` override vs default); response extraction from a
constructed `ConverseOutput`; timeout returns an error rather than hanging.
**Status**: Not Started

## Stage 5: `lambda` feature

**Goal**: `lambda::run(app_name, calling_module, verbosity, handler)` — installs the rustls
aws-lc-rs provider, sets up logging once at startup, then serves the handler.
**Success Criteria**: `cargo test --all-features` passes; clippy clean on all features.
**Tests**: the crypto provider install is idempotent across repeated calls.
**Status**: Not Started

## Out of scope

Consumer adoption. Those land as separate PRs in each repo once this merges.
