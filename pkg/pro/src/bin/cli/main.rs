#[cfg(feature = "tracing")]
use atty::{self, Stream};
#[cfg(feature = "tracing")]
use base::config::ENV_PREFIX;
use pro::config::Config;
#[cfg(feature = "tracing")]
use tracing_subscriber::{self, prelude::*, util::SubscriberInitExt};

#[cfg(feature = "telemetry")]
const SENTRY_DSN: &'static str = std::env!("SENTRY_DSN", "no $SENTRY_DSN set");

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    #[cfg(feature = "tracing")]
    let tracing_enabled = std::env::var(format!("{ENV_PREFIX}_ENABLE_TRACING")).is_ok();
    #[cfg(feature = "tracing")]
    {
        // TODO: handle --verbose/RUST_LOG setting
        let reg = tracing_subscriber::Registry::default().with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(atty::is(Stream::Stderr))
                .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                    match meta.module_path() {
                        Some(path) => path.starts_with(module_path!()) || path.starts_with("base"),
                        None => false,
                    }
                })),
        );
        #[cfg(feature = "telemetry")]
        if tracing_enabled {
            reg.with(sentry::integrations::tracing::layer()).init();
        } else {
            reg.init();
        }
        #[cfg(not(feature = "telemetry"))]
        reg.init();
    }
    #[cfg(feature = "telemetry")]
    let _guard = if std::env::var(format!("{ENV_PREFIX}_ENABLE_ERROR_REPORTING")).is_ok() {
        Some(sentry::init((
            SENTRY_DSN,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                auto_session_tracking: true,
                traces_sample_rate: if tracing_enabled {
                    1.0 // TODO: reduce
                } else {
                    0.0
                },
                enable_profiling: tracing_enabled,
                profiles_sample_rate: if tracing_enabled {
                    1.0 // TODO: reduce
                } else {
                    0.0
                },
                ..Default::default()
            },
        )))
    } else {
        None
    };
    let config = Config::new(&std::env::current_dir()?)?;
    Ok(())
}
