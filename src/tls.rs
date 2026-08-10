//! TLS backend selection.

/// Installs `aws-lc-rs` as the process-wide `rustls` crypto provider.
///
/// `rustls` refuses to build a TLS connection when more than one provider is
/// compiled in and none has been chosen, which surfaces as a runtime failure on
/// the first HTTPS request rather than at build time. Calling this before any
/// TLS work removes that failure mode.
///
/// Safe to call repeatedly and from anywhere: if a provider is already
/// installed, this leaves it alone. [`crate::lambda::run`] calls it for you;
/// binaries that aren't Lambdas should call it themselves at the top of `main`.
pub fn install_default_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
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
}
