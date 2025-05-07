use anyhow::Result;
use aws_config::ConfigLoader;
use aws_sdk_cloudwatch::types::{MetricDatum, StandardUnit};
use chrono::Utc;
use log::{LevelFilter, info, warn};
use std::borrow::Cow;
use std::str::FromStr;

const RUSTC_VERSION: &str = env!("RUSTC_VERSION");

pub fn set_up_logger<T>(app_name: T, calling_module: T, verbose: bool) -> Result<()>
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
        .level_for(app_name, level)
        .level_for("lambda_utils", level)
        .level_for(calling_module, level)
        .chain(std::io::stdout())
        .apply();

    info!("rustc: {RUSTC_VERSION}");

    Ok(())
}

fn parsed_rustc_version(rustc_version: &str) -> f64 {
    let rustc_version = rustc_version
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");

    f64::from_str(&rustc_version).unwrap_or(0.0)
}

pub async fn emit_rustc_metric<T>(app_name: T)
where
    T: Into<Cow<'static, str>>,
{
    let datum = MetricDatum::builder()
        .metric_name("RustcVersion")
        .value(parsed_rustc_version(RUSTC_VERSION))
        .unit(StandardUnit::Count)
        .build();

    let config = ConfigLoader::default().load().await;
    let client = aws_sdk_cloudwatch::Client::new(&config);

    if let Err(err) = client
        .put_metric_data()
        .namespace(app_name.into())
        .metric_data(datum)
        .send()
        .await
    {
        warn!("Failed to submit rustc metric: {err:?}");
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parsed_rustc_version() {
        assert_eq!(0.0, parsed_rustc_version("not-a-number"));
        assert_eq!(0.0, parsed_rustc_version("0.0"));
        assert_eq!(1.0, parsed_rustc_version("1.0"));
        assert_eq!(1.0, parsed_rustc_version("1"));
        assert_eq!(1.86, parsed_rustc_version("1.86"));
        assert_eq!(1.86, parsed_rustc_version("1.86.0"));
    }
}
