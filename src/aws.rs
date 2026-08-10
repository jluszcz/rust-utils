//! Shared AWS SDK configuration.

use aws_config::retry::RetryConfig;
use aws_config::{BehaviorVersion, Region, SdkConfig};

/// Retry attempts applied by [`config`], matching the AWS SDK's own standard
/// mode. Callers doing long bulk work want more; see
/// [`config_with_max_attempts`].
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

/// Loads the ambient AWS configuration with a standard retry policy.
///
/// `region` overrides the region that would otherwise be resolved from the
/// environment or profile — pass `None` to accept the ambient one, which is
/// what a Lambda always wants.
///
/// The retry policy is the reason to prefer this over building a
/// `ConfigLoader` directly: the SDK applies none unless asked, so every caller
/// that skipped it was one throttled request away from a hard failure.
pub async fn config(region: Option<String>) -> SdkConfig {
    config_with_max_attempts(region, DEFAULT_MAX_ATTEMPTS).await
}

/// [`config`] with an explicit retry budget.
///
/// Worth raising for long-running batch work, where being throttled partway
/// through is expected rather than exceptional.
pub async fn config_with_max_attempts(region: Option<String>, max_attempts: u32) -> SdkConfig {
    let mut loader = aws_config::defaults(BehaviorVersion::latest())
        .retry_config(RetryConfig::standard().with_max_attempts(max_attempts));

    if let Some(region) = region {
        loader = loader.region(Region::new(region));
    }

    loader.load().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_region_override_is_applied() {
        let config = config(Some("us-west-2".to_string())).await;

        assert_eq!(config.region().map(|r| r.as_ref()), Some("us-west-2"));
    }

    #[tokio::test]
    async fn test_default_retry_attempts_are_applied() {
        let config = config(Some("us-east-1".to_string())).await;

        assert_eq!(
            config.retry_config().map(|r| r.max_attempts()),
            Some(DEFAULT_MAX_ATTEMPTS)
        );
    }

    #[tokio::test]
    async fn test_retry_attempts_can_be_overridden() {
        let config = config_with_max_attempts(Some("us-east-1".to_string()), 10).await;

        assert_eq!(config.retry_config().map(|r| r.max_attempts()), Some(10));
    }
}
