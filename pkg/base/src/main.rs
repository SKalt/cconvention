use atty::{self, Stream};
use base::{config::ENV_PREFIX, log_info};

#[cfg(feature = "tracing")]
use tracing_subscriber::{self, prelude::*, util::SubscriberInitExt};

#[cfg(feature = "telemetry")]
const SENTRY_DSN: &'static str = std::env!("SENTRY_DSN", "no $SENTRY_DSN set");

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    // TODO: accept (-h|--help) and (-v|--version) flags
    #[cfg(feature = "tracing")]
    {
        let reg = tracing_subscriber::Registry::default().with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(atty::is(Stream::Stderr))
                .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                    match meta.module_path() {
                        Some(path) => path.starts_with(module_path!()),
                        None => false,
                    }
                })),
        );
        #[cfg(feature = "telemetry")]
        {
            if std::env::var(format!("{ENV_PREFIX}_DISABLE_TRACING")).is_err() {
            reg.with(sentry::integrations::tracing::layer()).init();
        } else {
            reg.init();
        };
        }
        #[cfg(not(feature = "telemetry"))]
        reg.init();
    }
    #[cfg(feature = "telemetry")]
    let _guard = if std::env::var(format!("{ENV_PREFIX}_DISABLE_ERROR_REPORTING")).is_err() {
        Some(sentry::init((
            SENTRY_DSN,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                auto_session_tracking: true,
                traces_sample_rate: 1.0, // TODO: reduce sampling rate
                enable_profiling: true,
                profiles_sample_rate: 1.0, // TODO: reduce sampling rate
                ..Default::default()
            },
        )))
    } else {
        None
    };
    let cfg = base::config::DefaultConfig::new();
    base::server::Server::from_stdio(cfg)
        .init(&base::server::CAPABILITIES)?
        .serve()?;
    log_info!("done");
    Ok(())
}
