use atty::{self, Stream};
use base::log_info;

#[cfg(feature = "tracing")]
use tracing_subscriber::{self, prelude::*, util::SubscriberInitExt};

#[cfg(feature = "telemetry")]
const SENTRY_DSN: &'static str = std::env!("SENTRY_DSN", "no $SENTRY_DSN set");

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    #[cfg(feature = "tracing")]
    {
        let reg = tracing_subscriber::Registry::default().with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(atty::is(Stream::Stderr)),
        );
        #[cfg(feature = "telemetry")]
        let reg = reg.with(sentry::integrations::tracing::layer());
        reg.init();
    }
    #[cfg(feature = "telemetry")]
    let _guard = sentry::init((
        SENTRY_DSN,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            auto_session_tracking: true,
            traces_sample_rate: 1.0,
            enable_profiling: true,
            profiles_sample_rate: 1.0,
            debug: true,
            ..Default::default()
        },
    ));
    base::server::Server::from_stdio(base::config::DefaultConfig)
        .init(&base::server::CAPABILITIES)?
        .serve()?;
    log_info!("done");
    Ok(())
}
