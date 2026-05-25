use crate::{Verbosity, set_up_logger};

pub async fn init(
    app_name: &'static str,
    calling_module: &'static str,
    verbosity: impl Into<Verbosity>,
) -> anyhow::Result<()> {
    set_up_logger(app_name, calling_module, verbosity.into())
}
