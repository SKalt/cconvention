#[macro_use]
extern crate lazy_static;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use atty::Stream;
use base::{config::ENV_PREFIX, log_info};
mod config;
mod lints;
#[cfg(feature = "tracing")]
use tracing_subscriber::{self, prelude::*, util::SubscriberInitExt};

use base::server::Server;

use crate::config::Config;

lazy_static! {
    static ref CAPABILITIES: lsp_types::ServerCapabilities = {
        let cap = base::server::CAPABILITIES.clone();
        // TODO: increased capabilities?
        cap
    };
}

#[cfg(feature = "telemetry")]
const SENTRY_DSN: &'static str = std::env!("SENTRY_DSN", "no $SENTRY_DSN set");

// now more on/off linting configuration
// IDEA: plugin system based on tree-sitter + counts of matches
// IDEA: Lint = Fn(&GitCommitDocument, code: &str, severity: &lsp_types::DiagnosticSeverity) -> Option<lsp_types::Diagnostic>
// https://commitlint.js.org/#/reference-rules
// 1234  1: already implemented; 2: neat 3: silly; 4: configurability: s=str, i=u8, b=bool
// 1  b body-leading-blank
// 1  b footer-leading-blank
// 1  b footer-empty
// 1  b scope-empty
// 1  b type-empty
// 1  b subject-empty
// 1  u header-max-length
// 1  u body-max-line-length
// 1  V type-enum
// 1  V scope-enum
// 1  b signed-off-by
// 1  b body-empty

//  2 b references-empty *********
//  2 V trailer-exists ***********

//  2 u body-min-length
//  2 u footer-min-length
//  2 s scope-case
//  2 u subject-min-length
//  2 s type-case
//    u body-max-length
//    u footer-max-line-length
//    u header-min-length
//    u scope-max-length ~~~~~~~~
//    u scope-min-length
//    s subject-case
//    u subject-max-length
//    u type-max-length
//    u type-min-length
//   3s header-case
//   3b header-full-stop
//   3b subject-exclamation-mark
//   3b subject-full-stop

struct ConfigStore_ {
    dirs: HashMap<PathBuf, Arc<dyn base::config::Config>>,
    tracing_enabled: bool,
    error_reporting_enabled: bool,
}
impl ConfigStore_ {
    fn new() -> Self {
        Self {
            dirs: HashMap::new(),
            tracing_enabled: std::env::var(format!("{ENV_PREFIX}_ENABLE_TRACING")).is_ok(),
            error_reporting_enabled: std::env::var(format!("{ENV_PREFIX}_ENABLE_ERROR_REPORTING"))
                .is_ok(),
        }
    }
}

impl base::config::ConfigStore for ConfigStore_ {
    fn get(
        &mut self,
        worktree_root: Option<PathBuf>,
    ) -> Result<Arc<dyn base::config::Config>, Box<dyn std::error::Error + Send + Sync>> {
        let worktree_root = worktree_root.unwrap_or(std::env::current_dir()?);

        if let Some(cfg) = self.dirs.get(&worktree_root) {
            Ok(cfg.to_owned())
        } else {
            let cfg = Arc::new(Config::new(&worktree_root)?);
            self.dirs.insert(worktree_root, cfg.clone());
            Ok(cfg)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let config_store = ConfigStore_::new();
    #[cfg(feature = "tracing")]
    {
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
        if config_store.tracing_enabled {
            reg.with(sentry::integrations::tracing::layer()).init();
        } else {
            reg.init();
        }
        #[cfg(not(feature = "telemetry"))]
        reg.init();
    }
    #[cfg(feature = "telemetry")]
    if config_store.error_reporting_enabled {
        let _guard = sentry::init((
            SENTRY_DSN,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                auto_session_tracking: true,
                traces_sample_rate: if config_store.tracing_enabled {
                    1.0 // TODO: reduce
                } else {
                    0.0
                },
                enable_profiling: config_store.tracing_enabled,
                profiles_sample_rate: if config_store.tracing_enabled {
                    1.0 // TODO: reduce
                } else {
                    0.0
                },
                ..Default::default()
            },
        ));
    }
    Server::from_stdio(config_store)
        .init(&base::server::CAPABILITIES)?
        .serve()?;
    log_info!("done");
    Ok(())
}
