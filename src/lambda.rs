//! Entry-point helper for AWS Lambda binaries.

use crate::{Verbosity, set_up_logger};

/// Prepares a Lambda invocation: currently just logging setup.
///
/// Call this once at the top of `main`, before `lambda_runtime::run`. It is
/// `async` and returns a `Result` because Lambda initialization has needed both
/// before and may again — the handful of binaries that call it are easier to
/// leave as-is than to churn every time this crate's needs change.
pub async fn init(
    app_name: &'static str,
    calling_module: &'static str,
    verbosity: impl Into<Verbosity>,
) -> anyhow::Result<()> {
    set_up_logger(app_name, calling_module, verbosity)
}
