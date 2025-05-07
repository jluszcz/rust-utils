use anyhow::Result;
use chrono::Utc;
use log::{LevelFilter, info};
use std::borrow::Cow;

const RUSTC_VERSION: &str = env!("RUSTC_VERSION");

pub fn set_up_logger<T>(application: T, calling_module: T, verbose: bool) -> Result<()>
where
    T: Into<Cow<'static, str>>,
{
    let level = if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

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
        .level_for(application, level)
        .level_for("lambda_utils", level)
        .level_for(calling_module, level)
        .chain(std::io::stdout())
        .apply();

    info!("rustc: {RUSTC_VERSION}");

    Ok(())
}
