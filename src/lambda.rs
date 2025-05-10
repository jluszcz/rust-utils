use std::str::FromStr;
use aws_config::ConfigLoader;
use aws_sdk_cloudwatch::types::{MetricDatum, StandardUnit};
use log::warn;
use crate::set_up_logger;

const RUSTC_VERSION: &str = env!("RUSTC_VERSION");

pub async fn init(
    app_name: &'static str,
    calling_module: &'static str,
    verbose: bool,
) -> anyhow::Result<()> {
    set_up_logger(app_name, calling_module, verbose)?;
    emit_rustc_metric(app_name).await;

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

pub async fn emit_rustc_metric(app_name: &'static str) {
    let datum = MetricDatum::builder()
        .metric_name("RustcVersion")
        .value(parsed_rustc_version(RUSTC_VERSION))
        .unit(StandardUnit::Count)
        .build();

    let config = ConfigLoader::default().load().await;
    let client = aws_sdk_cloudwatch::Client::new(&config);

    if let Err(err) = client
        .put_metric_data()
        .namespace(app_name)
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
