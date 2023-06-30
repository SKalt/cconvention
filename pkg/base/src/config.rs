use regex::Regex;
use std::{collections::HashMap, path::PathBuf};

/// use this for reading configuration from the environment
pub const ENV_PREFIX: &str = "GIT_CC_LS";

use crate::{
    document::{
        lints::{construct_default_lint_tests_map, LintConfig, LintFn},
        GitCommitDocument,
    },
    git::{get_worktree_root, git, to_path},
};

lazy_static! {
    static ref RE: Regex =
        Regex::new(r"^(?P<type>[^:\(!]+)(?:\((?P<scope>[^\)]+)\))?:\s*(?P<subject>.+)$").unwrap();
}
pub trait Config: LintConfig {
    /// get the repo root for a given file URL
    /// most implementations of Config should cache this
    fn repo_root_for(&mut self, url: &lsp_types::Url) -> Option<PathBuf> {
        let path = to_path(url).ok()?;
        get_worktree_root(&path).ok()
    }
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
    fn scope_suggestions(&mut self, url: &lsp_types::Url) -> Vec<(String, String)> {
        // guess the scopes from the staged files
        let worktree_root = self.repo_root_for(url);
        let staged_files: Vec<String> = git(
            &["--no-pager", "diff", "--name-only", "--cached"],
            worktree_root.clone(),
        )
        .unwrap_or("".into())
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.to_owned())
        .collect();
        let applicable_scopes: Vec<(String, String)> = {
            let mut args = vec!["log", "--format=%s", "--max-count=1000", "--"];
            args.extend(staged_files.iter().map(|s| s.as_str()));
            let output = git(args.as_slice(), worktree_root).unwrap_or("".into());
            let unique: HashMap<&str, u64> = output
                .lines()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty())
                .filter_map(|line| RE.captures(line))
                .filter_map(|captures| captures.name("scope"))
                .filter_map(|scope| Some(scope.as_str()))
                .fold(HashMap::<&str, u64>::new(), |mut set, scope| {
                    if let Some(count) = set.get_mut(scope) {
                        *count += 1;
                    } else {
                        set.insert(scope, 1);
                    };
                    set
                });
            let mut sorted_descending: Vec<(&str, u64)> = unique.into_iter().collect();
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

const DEFAULT_TYPES: &[(&str, &str)] = &[
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
// TODO: disabling tracing, error reporting
pub struct DefaultConfig {
    git_worktree_roots: HashMap<lsp_types::Url, PathBuf>,
    tests: HashMap<
        &'static str,
        Box<dyn Fn(&GitCommitDocument) -> Vec<lsp_types::Diagnostic> + 'static>,
    >,
}
impl DefaultConfig {
    pub fn new() -> Self {
        DefaultConfig {
            git_worktree_roots: HashMap::with_capacity(1),
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
    fn repo_root_for(&mut self, url: &lsp_types::Url) -> Option<PathBuf> {
        if let Some(root) = self.git_worktree_roots.get(&url) {
            return Some(root.clone());
        }
        let root = get_worktree_root(&to_path(&url).ok()?).ok()?;
        self.git_worktree_roots.insert(url.to_owned(), root.clone());
        Some(root)
    }
}
