use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[cfg(feature = "tracing")]
use base::config::ENV_PREFIX;

use base::{cli::cli, document::linting::utils::construct_default_lint_tests_map};

pub struct DefaultConfigStore(DefaultConfig);
impl DefaultConfigStore {
    pub fn new() -> Self {
        DefaultConfigStore(DefaultConfig::new())
    }
}

impl Default for DefaultConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

impl base::config::ConfigStore for DefaultConfigStore {
    /// always returns a clone of the same DefaultConfig for each worktree_root
    fn get(
        &mut self,
        worktree_root: Option<PathBuf>,
    ) -> Result<Arc<dyn base::config::Config>, Box<dyn std::error::Error + Send + Sync>> {
        let mut cfg = self.0.clone();
        cfg.worktree_root = worktree_root;
        Ok(Arc::new(cfg))
    }
}

#[derive(Clone)]
pub struct DefaultConfig {
    worktree_root: Option<PathBuf>,
    tests: HashMap<&'static str, Arc<base::document::linting::LintFn<'static>>>,
}

impl DefaultConfig {
    pub fn new() -> Self {
        DefaultConfig {
            worktree_root: None,
            tests: construct_default_lint_tests_map(50),
        }
    }
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl base::document::linting::LintConfig for DefaultConfig {
    fn worktree_root(&self) -> Option<PathBuf> {
        self.worktree_root.clone()
    }
    fn get_test(&self, code: &str) -> Option<&Arc<base::document::linting::LintFn>> {
        self.tests.get(code)
    }
}
impl base::config::Config for DefaultConfig {}

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    cli(
        || Ok(DefaultConfigStore::new()),
        &base::server::CAPABILITIES,
        #[cfg(feature = "tracing")]
        std::env::var(format!("{ENV_PREFIX}_DISABLE_TRACING")).is_err(),
        #[cfg(feature = "telemetry")]
        std::env::var(format!("{ENV_PREFIX}_DISABLE_ERROR_REPORTING")).is_err(),
    )
}
