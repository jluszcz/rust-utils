//! TLS backend selection.

/// Installs the process-wide `rustls` crypto provider.
///
/// Which provider depends on the enabled feature: `tls` installs `aws-lc-rs`,
/// `tls-ring` installs `ring`, and `tls` wins when both are on. Without a
/// provider installed, building an HTTPS client panics rather than the build
/// failing. Calling this before any TLS work removes that failure mode.
///
/// Safe to call repeatedly and from anywhere: if a provider is already
/// installed, this leaves it alone. [`crate::lambda::run`] calls it for you;
/// binaries that aren't Lambdas — including any that take `query` without
/// `lambda` — call it themselves at the top of `main`.
pub fn install_default_provider() {
    #[cfg(feature = "tls")]
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    #[cfg(all(feature = "tls-ring", not(feature = "tls")))]
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_is_idempotent_and_leaves_a_provider() {
        install_default_provider();
        install_default_provider();

        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }

    /// The installed provider is the one the enabled feature names.
    ///
    /// `tls` wins when both are on, which is what keeps a consumer that adds
    /// `tls-ring` to an existing `lambda` build on the backend it already had.
    #[test]
    fn test_installed_provider_matches_the_enabled_feature() {
        install_default_provider();
        let provider =
            rustls::crypto::CryptoProvider::get_default().expect("a provider is installed");

        #[cfg(feature = "tls")]
        let expected = rustls::crypto::aws_lc_rs::default_provider();
        #[cfg(all(feature = "tls-ring", not(feature = "tls")))]
        let expected = rustls::crypto::ring::default_provider();

        assert_eq!(
            provider.cipher_suites.len(),
            expected.cipher_suites.len(),
            "installed provider is not the one the enabled feature names"
        );
    }
}
