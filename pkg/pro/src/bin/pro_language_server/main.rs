#[macro_use]
extern crate lazy_static;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use atty::Stream;
use base::{config::ENV_PREFIX, log_debug, log_info};

#[cfg(feature = "tracing")]
use tracing_subscriber::{self, prelude::*, util::SubscriberInitExt};

use base::server::Server;

use pro::config::Config;

fn construct_capabilities() -> lsp_types::ServerCapabilities {
    let mut capabilities = base::server::CAPABILITIES.clone();
    let filter = Some(lsp_types::FileOperationRegistrationOptions {
        filters: vec![lsp_types::FileOperationFilter {
            scheme: Some("file".to_string()),
            pattern: lsp_types::FileOperationPattern {
                matches: Some(lsp_types::FileOperationPatternKind::File),
                options: Some(lsp_types::FileOperationPatternOptions {
                    ignore_case: Some(false),
                }),
                glob: CONFIG_FILE_GLOB.clone(), // ..Default::default()
            },
        }],
    });
    capabilities.workspace = Some(lsp_types::WorkspaceServerCapabilities {
        workspace_folders: None,
        file_operations: Some(lsp_types::WorkspaceFileOperationsServerCapabilities {
            did_create: filter.clone(),
            will_create: filter.clone(),
            did_rename: filter.clone(),
            will_rename: filter.clone(),
            did_delete: filter.clone(),
            will_delete: filter,
        }),
    });
    capabilities
}

lazy_static! {
    static ref CAPABILITIES: lsp_types::ServerCapabilities = construct_capabilities();
    static ref CONFIG_FILE_GLOB: String = {
        let mut exts = vec!["json"];

        #[cfg(feature = "toml_config")]
        exts.push("toml");

        format!("**/commit_convention.{}", exts.join(","))
    };
}

#[cfg(feature = "telemetry")]
const SENTRY_DSN: &'static str = std::env!("SENTRY_DSN", "no $SENTRY_DSN set");

struct ConfigStore_ {
    dirs: HashMap<PathBuf, Arc<dyn base::config::Config>>,
    #[cfg(feature = "tracing")]
    tracing_enabled: bool,
    #[cfg(feature = "telemetry")]
    error_reporting_enabled: bool,
}
impl ConfigStore_ {
    fn new() -> Self {
        Self {
            dirs: HashMap::new(),
            #[cfg(feature = "tracing")]
            tracing_enabled: std::env::var(format!("{ENV_PREFIX}_ENABLE_TRACING")).is_ok(),
            #[cfg(feature = "telemetry")]
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
    fn set_dirty(&mut self, paths: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut roots = Vec::with_capacity(paths.len());
        for path in paths {
            // since we're looking in ${root}/.config/ and ${root}, grab the parent and grandparent dirs
            // shouldn't panic even if the repo root is located in /
            let worktree_root = path
                .parent()
                .map(|parent| {
                    let parent: PathBuf = parent.into();
                    if self.dirs.contains_key(&parent) {
                        Some(parent)
                    } else {
                        if let Some(grandparent) = parent.parent() {
                            let grandparent: PathBuf = grandparent.into();
                            if self.dirs.contains_key(&grandparent) {
                                Some(grandparent)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                })
                .flatten();
            if let Some(worktree_root) = worktree_root {
                log_debug!(
                    "invalidating config cache for {:?} with worktree root {:?}",
                    path,
                    worktree_root
                );
                if let Ok(cfg) = Config::new(&worktree_root) {
                    self.dirs.insert(worktree_root.clone(), Arc::new(cfg));
                } else {
                    self.dirs.remove(&worktree_root); // handle error on next access
                }
                roots.push(worktree_root);
            }
        }
        roots
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
        .init(&CAPABILITIES)?
        .serve()?;
    log_info!("done");
    Ok(())
}
