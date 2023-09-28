use std::{path::PathBuf, sync::Arc};

use super::GitCommitDocument;
pub mod default;
pub mod utils;
/// a fatal parse error according to the conventional commit spec
pub const INVALID: &str = "INVALID";

/// a lint-fn is a test that can return zero to many logically equivalent diagnostics
/// differentiated by a message: e.g. `[line-too-long, line-too-short]`
pub type LintFn<'cfg> = dyn Fn(&GitCommitDocument) -> Vec<lsp_types::Diagnostic> + 'cfg;

pub trait LintConfig {
    /// provides information to the user about where the lint configuration came from.
    fn source(&self) -> &str {
        "<default>"
    }
    fn worktree_root(&self) -> Option<PathBuf>;
    fn enabled_lint_codes(&self) -> Vec<&str> {
        Vec::from(default::ENABLED_LINTS)
    }
    fn lint_severity(&self, lint_code: &str) -> &lsp_types::DiagnosticSeverity {
        default::LINT_SEVERITY
            .get(lint_code)
            .unwrap_or(&lsp_types::DiagnosticSeverity::WARNING)
    }

    // fn lint_tests(&self) -> &HashMap<&str, Box<LintFn>>;
    fn get_test(&self, code: &str) -> Option<&Arc<LintFn>>;
    fn lint(&self, doc: &GitCommitDocument) -> Vec<lsp_types::Diagnostic> {
        log_debug!("linting document: {}", doc.code);
        let mut diagnostics = doc.get_mandatory_lints();
        log_debug!(
            "mandatory diagnostics: {:?}",
            diagnostics
                .iter()
                .map(|d| d.code.as_ref().unwrap())
                .collect::<Vec<_>>()
        );
        // let code_map = construct_default_lint_tests_map(self);
        // for code in self.enabled_lint_codes() {}
        diagnostics.extend(
            self.enabled_lint_codes()
                .iter()
                .filter_map(|code| {
                    let test = self.get_test(code);
                    if test.is_none() {
                        log_debug!("Missing test for code {:?}", code);
                    }
                    test
                })
                .map(|f| f(doc))
                .map(|mut v| {
                    for diagnostic in v.iter_mut() {
                        if diagnostic.severity.is_none() {
                            match &diagnostic.code {
                                Some(lsp_types::NumberOrString::String(code)) => {
                                    diagnostic.severity = Some(self.lint_severity(code).to_owned());
                                }
                                Some(bad_code) => {
                                    panic!("Unsupported numeric code: {:?}", bad_code)
                                }
                                None => panic!("missing code"),
                            }
                        }
                    }
                    v
                })
                .reduce(|mut acc, red| {
                    acc.extend(red);
                    acc
                })
                .unwrap_or(vec![]),
        );
        diagnostics
    }
}
