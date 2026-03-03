//! Observability.

use std::sync::{Arc, OnceLock};

use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    EnvFilter, Layer as _, Registry, fmt::format::FmtSpan,
    layer::SubscriberExt as _,
};

use crate::{cli::LogFormat, error};

mod layer;
pub(crate) mod prelude;

/// Initialize observability.
pub(crate) fn init(
    longest_name: Arc<OnceLock<usize>>,
    log_format: LogFormat,
) -> Result<(), error::Observability> {
    use error::Observability as E;

    let make_layer = || {
        tracing_subscriber::fmt::layer()
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
    };

    let make_env_filter = || {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env()
            .map_err(|e| E::FromEnvError(e.into()))
    };

    let layer = match log_format {
        LogFormat::Default => layer::Impl::new(longest_name).boxed(),
        LogFormat::Full => make_layer().with_filter(make_env_filter()?).boxed(),
        LogFormat::Compact => {
            make_layer().compact().with_filter(make_env_filter()?).boxed()
        }
        LogFormat::Pretty => {
            make_layer().pretty().with_filter(make_env_filter()?).boxed()
        }
        LogFormat::Json => {
            make_layer().json().with_filter(make_env_filter()?).boxed()
        }
    };

    let subscriber = Registry::default().with(layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("should be able to set global default tracing subscriber");

    Ok(())
}
