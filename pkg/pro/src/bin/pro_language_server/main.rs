#[macro_use]
extern crate lazy_static;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use base::{cli::cli, config::ENV_PREFIX, log_debug};

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

struct ConfigStore_ {
    dirs: HashMap<PathBuf, Arc<dyn base::config::Config>>,
}
impl ConfigStore_ {
    fn new() -> Self {
        Self {
            dirs: HashMap::new(),
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
            let worktree_root = path.parent().and_then(|parent| {
                let parent: PathBuf = parent.into();
                if self.dirs.contains_key(&parent) {
                    Some(parent)
                } else if let Some(grandparent) = parent.parent() {
                    let grandparent: PathBuf = grandparent.into();
                    if self.dirs.contains_key(&grandparent) {
                        Some(grandparent)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
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
    cli(
        || Ok(ConfigStore_::new()),
        &CAPABILITIES,
        #[cfg(feature = "tracing")]
        std::env::var(format!("{ENV_PREFIX}_ENABLE_TRACING")).is_ok(),
        #[cfg(feature = "telemetry")]
        std::env::var(format!("{ENV_PREFIX}_ENABLE_ERROR_REPORTING")).is_ok(),
    )
}
