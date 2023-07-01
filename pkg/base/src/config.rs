use regex::Regex;
use std::{collections::HashMap, path::PathBuf};

/// use this for reading configuration from the environment
pub const ENV_PREFIX: &str = "GIT_CC_LS";

use crate::document::{
    lints::{construct_default_lint_tests_map, LintConfig, LintFn},
    GitCommitDocument,
};
use crate::git;

lazy_static! {
    static ref RE: Regex =
        Regex::new(r"^(?P<type>[^:\(!]+)(?:\((?P<scope>[^\)]+)\))?:\s*(?P<subject>.+)$").unwrap();
}

pub const DEFAULT_TYPES: &[(&str, &str)] = &[
    ("feat", "adds a new feature"),
    ("fix", "fixes a bug"),
    ("docs", "changes only the documentation"),
    (
        "style",
        "changes the style but not the meaning of the code (such as formatting)",
    ),
    ("perf", "improves performance"),
    ("test", "adds or corrects tests"),
    ("build", "changes the build system or external dependencies"),
    ("chore", "changes outside the code, docs, or tests"),
    ("ci", "changes to the Continuous Integration (CI) system"),
    ("refactor", "changes the code without changing behavior"),
    ("revert", "reverts prior changes"),
];

/// provides
pub trait Config: LintConfig {
    /// the source of the configuration
    fn source(&self) -> &str;
    // TODO: ^change to PathBuf or lsp_types::Url
    // TODO: ^consider removing in favor of a `search_path` method or similar?
    fn type_suggestions(&self) -> Vec<(String, String)> {
        let mut result = Vec::with_capacity(DEFAULT_TYPES.len());
        for (label, detail) in DEFAULT_TYPES {
            result.push((label.to_string(), detail.to_string()));
        }
        result
    }
    fn scope_suggestions(&self, worktree_root: Option<PathBuf>) -> Vec<(String, String)> {
        // guess the scopes from the staged files
        let files = git::staged_files(worktree_root.clone());
        let applicable_scopes: Vec<(String, String)> = {
            let output = git::related_commits(files.as_slice(), worktree_root.clone());
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

// TODO: disabling tracing, error reporting
pub struct DefaultConfig {
    tests: HashMap<
        &'static str,
        Box<dyn Fn(&GitCommitDocument) -> Vec<lsp_types::Diagnostic> + 'static>,
    >,
}

impl DefaultConfig {
    pub fn new() -> Self {
        DefaultConfig {
            tests: construct_default_lint_tests_map(50),
        }
    }
}

impl LintConfig for DefaultConfig {
    fn get_test(&self, code: &str) -> Option<&Box<LintFn>> {
        self.tests.get(code)
    }
}
impl Config for DefaultConfig {
    fn source(&self) -> &str {
        "<default>"
    }
}
