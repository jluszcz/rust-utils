//! Entry-point helper for AWS Lambda binaries.

use crate::{Verbosity, set_up_logger};

#[cfg(feature = "lambda")]
use lambda_runtime::{LambdaEvent, service_fn};
#[cfg(feature = "lambda")]
use serde::{Serialize, de::DeserializeOwned};
#[cfg(feature = "lambda")]
use std::future::Future;

/// Runs `handler` as a Lambda function until the runtime shuts down.
///
/// Replaces the `main` that each Lambda binary wrote by hand. Before serving,
/// it installs the `rustls` crypto provider (see
/// [`crate::tls::install_default_provider`]) and sets up logging — once at cold
/// start, rather than on every invocation, which is where the hand-written
/// versions disagreed with each other.
///
/// # Errors
///
/// Returns an error if logging setup fails or the Lambda runtime does.
#[cfg(feature = "lambda")]
pub async fn run<T, R, F, Fut>(
    app_name: &'static str,
    calling_module: &'static str,
    verbosity: impl Into<Verbosity>,
    handler: F,
) -> Result<(), lambda_runtime::Error>
where
    T: DeserializeOwned,
    R: Serialize,
    F: Fn(LambdaEvent<T>) -> Fut,
    Fut: Future<Output = Result<R, lambda_runtime::Error>>,
{
    crate::tls::install_default_provider();
    set_up_logger(app_name, calling_module, verbosity)?;

    lambda_runtime::run(service_fn(handler)).await
}

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

#[cfg(test)]
#[cfg(feature = "lambda")]
mod tests {
    use super::*;
    use lambda_runtime::LambdaEvent;
    use serde_json::{Value, json};

    #[test]
    fn test_run_accepts_a_json_handler() {
        async fn handler(_event: LambdaEvent<Value>) -> Result<Value, lambda_runtime::Error> {
            Ok(json!({}))
        }

        // Constructing the future is the assertion: it proves the generic
        // bounds accept the handler shape every consumer uses. Awaiting it
        // would start polling the Lambda runtime API.
        let _future = run("app", module_path!(), false, handler);
    }

    #[test]
    fn test_run_accepts_a_typed_handler() {
        async fn handler(event: LambdaEvent<String>) -> Result<String, lambda_runtime::Error> {
            Ok(event.payload)
        }

        let _future = run("app", module_path!(), 2, handler);
    }
}
