use anyhow::Result;
use chrono::Utc;
use log::LevelFilter;

pub mod lambda;

pub enum Verbosity {
    Trace,
    Debug,
    Info
}

impl From<u8> for Verbosity {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Debug,
            2 => Self::Trace,
            _ => Self::Info,
        }
    }
}

impl From<bool> for Verbosity {
    fn from(value: bool) -> Self {
        if value {
            Self::Debug
        } else {
            Self::Info
        }
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

pub fn set_up_logger(
    app_name: &'static str,
    calling_module: &'static str,
    verbosity: Verbosity
) -> Result<()> {
    let level = verbosity.into();

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
        .level_for("lambda_utils", level)
        .chain(std::io::stdout())
        .apply();

    Ok(())
}
