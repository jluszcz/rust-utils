//! Small utilities shared across personal Rust projects: logging setup, an
//! AWS Lambda entry point, and (behind the `query` feature) an HTTP query
//! helper with an on-disk response cache.
#![warn(missing_docs)]

use anyhow::Result;
use chrono::Utc;
use log::{LevelFilter, info};

pub mod lambda;

#[cfg(feature = "query")]
pub mod query;

#[cfg(feature = "query")]
pub mod cache;

pub(crate) const RUSTC_VERSION: &str = env!("RUSTC_VERSION");

/// How much the calling application should log.
///
/// This maps onto [`log::LevelFilter`], but only over the three levels worth
/// exposing as a user-facing switch. Construct it from a `-v` count via
/// [`From<u8>`](#impl-From<u8>-for-Verbosity) or from a boolean flag.
#[derive(Debug, Copy, Clone)]
pub enum Verbosity {
    /// Everything, including per-request detail.
    Trace,
    /// Diagnostics useful while developing.
    Debug,
    /// The default: significant events only.
    Info,
}

impl From<u8> for Verbosity {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Info,
            1 => Self::Debug,
            _ => Self::Trace,
        }
    }
}

impl From<bool> for Verbosity {
    fn from(value: bool) -> Self {
        if value { Self::Debug } else { Self::Info }
    }
}

impl From<Verbosity> for LevelFilter {
    fn from(value: Verbosity) -> Self {
        match value {
            Verbosity::Trace => Self::Trace,
            Verbosity::Debug => Self::Debug,
            Verbosity::Info => Self::Info,
        }
    }
}

/// Installs a global logger that prints timestamped lines to stdout.
///
/// The level applies to `app_name`, `calling_module`, and this crate; everything
/// else — dependencies, most importantly — is capped at `Warn`, so raising
/// verbosity does not bury the application's own output in library chatter.
/// `calling_module` is normally `module_path!()` from the caller.
///
/// Calling this more than once is harmless: the second and later calls are
/// ignored rather than failing, so tests and Lambda cold starts can both call it
/// freely.
///
/// # Errors
///
/// Returns `Ok(())` today; the signature is fallible so a future implementation
/// can report a failure without breaking callers.
pub fn set_up_logger(
    app_name: &'static str,
    calling_module: &'static str,
    verbosity: impl Into<Verbosity>,
) -> Result<()> {
    let level = LevelFilter::from(verbosity.into());

    let _ = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] [{}] {}",
                Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(LevelFilter::Warn)
        .level_for(app_name, level)
        .level_for(calling_module, level)
        .level_for(env!("CARGO_CRATE_NAME"), level)
        .chain(std::io::stdout())
        .apply();

    info!("rustc version: {RUSTC_VERSION}");

    Ok(())
}
