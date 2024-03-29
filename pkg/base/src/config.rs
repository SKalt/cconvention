// © Steven Kalt
// SPDX-License-Identifier: APACHE-2.0
use regex::Regex;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
/// use this for reading configuration from the environment
pub const ENV_PREFIX: &str = "GIT_CC_LS";

use crate::document::linting::LintConfig;
use crate::git;

pub const DEFAULT_TYPES: &[(&str, &str)] = &[
    ("feat", "Adds a new feature."),
    ("fix", "Fixes a bug."),
    ("docs", "Changes only the documentation."),
    (
        "style",
        "Changes the style but not the meaning of the code (such as formatting).",
    ),
    ("perf", "Improves performance."),
    ("test", "Adds or corrects tests."),
    (
        "build",
        "Changes the build system or external dependencies.",
    ),
    ("chore", "Changes outside the code, docs, or tests."),
    ("ci", "Changes to the Continuous Integration (CI) system."),
    ("refactor", "Changes the code without changing behavior."),
    ("revert", "Reverts prior changes."),
    ("temp", "A commit to be fixed/rebased later."),
];

lazy_static! {
    static ref RE: Regex =
        Regex::new(r"^(?P<type>[^:\(!]+)(?:\((?P<scope>[^\)]+)\))?:\s*(?P<subject>.+)$").unwrap();
}

/// provides
pub trait Config: LintConfig {
    // TODO: ^change to PathBuf or lsp_types::Url
    // TODO: ^consider removing in favor of a `search_path` method or similar?
    fn type_suggestions(&self) -> Vec<(String, String)> {
        let mut result = Vec::with_capacity(DEFAULT_TYPES.len());
        for (label, detail) in DEFAULT_TYPES {
            result.push((label.to_string(), detail.to_string()));
        }
        result
    }
    fn scope_suggestions(&self) -> Vec<(String, String)> {
        // guess the scopes from the staged files
        let worktree_root = self.worktree_root();
        let files = git::staged_files(worktree_root.clone());
        let applicable_scopes: Vec<(String, String)> = {
            let output = git::related_commits(files.as_slice(), worktree_root);
            let unique: HashMap<&str, usize> = output
                .iter()
                .filter_map(|line| RE.captures(line))
                .filter_map(|captures| captures.name("scope"))
                .filter_map(|scope| Some(scope.as_str()))
                // using an integer smaller than usize won't matter, since we're iterating
                // over tuples of `(&str, _)` later which have alignment on usize boundaries.
                .fold(HashMap::<&str, usize>::new(), |mut set, scope| {
                    if let Some(count) = set.get_mut(scope) {
                        *count += 1;
                    } else {
                        set.insert(scope, 1);
                    };
                    set
                });
            let mut sorted_descending: Vec<(&str, usize)> = unique.into_iter().collect();
            sorted_descending.sort_by(|a, b| b.1.cmp(&a.1));
            sorted_descending
                .into_iter()
                .map(|(scope, count)| {
                    (
                        scope.to_owned(),
                        format!("used {} times in the currently-staged files", count),
                    )
                })
                .collect()
        };
        let mut result = Vec::with_capacity(applicable_scopes.len());

        for (label, detail) in applicable_scopes {
            result.push((label, detail));
        }
        result
    }
}

pub(crate) fn as_completion(items: &[(String, String)]) -> Vec<lsp_types::CompletionItem> {
    let mut result = Vec::with_capacity(items.len());
    for (label, detail) in items {
        let mut item = lsp_types::CompletionItem::new_simple(label.to_owned(), detail.to_owned());
        item.kind = Some(lsp_types::CompletionItemKind::ENUM_MEMBER);
        result.push(item);
    }
    result
}

pub trait ConfigStore {
    /// get the configuration relevant to the given worktree root
    fn get(
        &mut self,
        worktree_root: Option<PathBuf>,
    ) -> Result<Arc<dyn Config>, Box<dyn std::error::Error + Send + Sync>>;
    // self has to ^ be mutable because we might need to update the cache of configurations
    /// mark the given paths as dirty, returning the paths associated with invalidated configuration
    /// in order to reload them and optionally push updates to affected lints
    fn set_dirty(&mut self, _paths: Vec<PathBuf>) -> Vec<PathBuf> {
        vec![]
    }
}
