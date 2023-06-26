use regex::Regex;
use std::{collections::HashMap, path::PathBuf};

use crate::document::{
    lints::{construct_default_lint_tests_map, LintConfig, LintFn},
    GitCommitDocument,
};

lazy_static! {
    static ref RE: Regex =
        Regex::new(r"^(?P<type>[^:\(!]+)(?:\((?P<scope>[^\)]+)\))?:\s*(?P<subject>.+)$").unwrap();
}
pub trait Config: LintConfig {
    fn repo_root(&self) -> Option<PathBuf> {
        if let Ok(path) = std::process::Command::new("git")
            .arg("rev-parse")
            .arg("--show-toplevel")
            .output()
            .map(|output| {
                std::str::from_utf8(&output.stdout)
                    .unwrap()
                    .trim()
                    .to_owned()
            })
            .map(|path| PathBuf::from(path))
        {
            return Some(path);
        } else {
            return None;
        }
    }
    /// the source of the configuration
    fn source(&self) -> &str; // TODO: change to PathBuf or lsp_types::Url
    fn type_suggestions(&self) -> Vec<(String, String)> {
        let mut result = Vec::with_capacity(DEFAULT_TYPES.len());
        for (label, detail) in DEFAULT_TYPES {
            result.push((label.to_string(), detail.to_string()));
        }
        result
    }
    fn scope_suggestions(&self) -> Vec<(String, String)> {
        // guess the scopes from the staged files
        let cwd = self.repo_root().unwrap_or_else(|| PathBuf::from("."));
        let staged_files: Vec<String> = std::process::Command::new("git")
            .current_dir(cwd.clone())
            .arg("--no-pager")
            .arg("diff")
            .arg("--name-only")
            .arg("--cached")
            .output()
            .map(|output| {
                std::str::from_utf8(&output.stdout)
                    .map(|s| s.to_string())
                    .unwrap_or("".into())
            })
            .unwrap_or("".into())
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.len() > 0 {
                    Some(trimmed.to_owned())
                } else {
                    None
                }
            })
            .collect();
        let applicable_scopes: Vec<(String, String)> = {
            let mut cmd = std::process::Command::new("git");
            cmd.current_dir(cwd)
                .arg("--no-pager")
                .arg("log")
                .arg("--format=%s")
                .arg("--max-count=1000")
                .arg("--")
                .args(staged_files);

            let output = cmd
                .output()
                .map(|output| {
                    std::str::from_utf8(&output.stdout)
                        .map(|s| s.to_string())
                        .unwrap_or("".into())
                })
                .unwrap_or("".into());
            let unique: HashMap<&str, u64> = output
                .lines()
                .map(|line| line.trim())
                .filter(|line| line.len() > 0)
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
        // log_debug!("applicable scopes: {:?}", applicable_scopes);

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
