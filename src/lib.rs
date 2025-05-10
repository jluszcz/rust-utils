use anyhow::Result;
use chrono::Utc;
use log::LevelFilter;

pub mod lambda;

pub fn set_up_logger(
    app_name: &'static str,
    calling_module: &'static str,
    verbose: bool,
) -> Result<()> {
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
        .level_for(app_name, level)
        .level_for(calling_module, level)
        .level_for("lambda_utils", level)
        .chain(std::io::stdout())
        .apply();

    Ok(())
}
