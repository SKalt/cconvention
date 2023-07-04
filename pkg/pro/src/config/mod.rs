use super::lints;
use base::{
    document::{
        lints::{check_body_line_length, check_subject_line_length},
        GitCommitDocument,
    },
    log_debug,
};
use indexmap::IndexMap;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
mod git;
// TODO: move json_ish behind a feature flag
pub(crate) mod json_ish;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Severity {
    Error,
    Warning,
    Info,
    Hint,
    None,
}

impl From<Severity> for Option<lsp_types::DiagnosticSeverity> {
    fn from(value: Severity) -> Self {
        match value {
            Severity::Error => Some(lsp_types::DiagnosticSeverity::ERROR),
            Severity::Warning => Some(lsp_types::DiagnosticSeverity::WARNING),
            Severity::Info => Some(lsp_types::DiagnosticSeverity::INFORMATION),
            Severity::Hint => Some(lsp_types::DiagnosticSeverity::HINT),
            Severity::None => None,
        }
    }
}
impl Default for Severity {
    fn default() -> Self {
        Severity::Warning
    }
}

#[derive(Default)]
pub(crate) struct Config {
    worktree_root: PathBuf,
    types: IndexMap<String, String>,
    scopes: IndexMap<String, String>,
    severity: HashMap<String, lsp_types::DiagnosticSeverity>,
    enabled_lints: Vec<String>,
    // queries: HashMap<String, tree_sitter::Query>,
    tests: HashMap<String, Arc<dyn Fn(&GitCommitDocument) -> Vec<lsp_types::Diagnostic>>>,
}

impl Config {
    /// Load a config from the given worktree directory, adding default types, lints, & lint severity.
    pub(crate) fn new(
        worktree_root: &PathBuf,
        // query_cache: &mut HashMap<String, tree_sitter::Query>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use base::document::lints;
        // IDEA: draw lint-fn closures from a long-lived default store
        let (json, _file) = json_ish::get_config(worktree_root)?;
        let jfc = json.clone();
        let enabled_lints: Vec<String> = lints::DEFAULT_LINTS
            .iter()
            .chain(&["body_line_max_length"])
            .map(|lint_code| lint_code.to_string())
            .collect();
        let types_are_missing = json
            .types
            .as_ref()
            .map(|type_enum| type_enum.is_empty())
            .unwrap_or(true);
        let types = if types_are_missing {
            base::config::DEFAULT_TYPES
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        } else {
            json.types.clone().unwrap()
        };
        let scopes = json
            .scopes
            .as_ref()
            .map(|scopes| scopes.clone())
            .unwrap_or_default();
        let mut cfg = Config {
            worktree_root: worktree_root.clone(),
            enabled_lints,
            types: types.clone(), // TODO: figure out how to re-use cfg.types in enum-checking lint-fn
            scopes: scopes.clone(), // TODO: figure out how to re-use cfg.scopes in enum-checking lint-fn
            severity: HashMap::with_capacity(2),
            tests: HashMap::new(),
        };
        cfg.severity.insert(
            lints::TYPE_ENUM.to_string(),
            if types_are_missing {
                lsp_types::DiagnosticSeverity::HINT
            } else {
                lsp_types::DiagnosticSeverity::ERROR
            },
        );
        cfg.tests.insert(
            lints::TYPE_ENUM.to_string(),
            Arc::new(move |doc: &GitCommitDocument| {
                let mut lints = vec![];
                doc.subject.as_ref().map(|header| {
                    let type_text = header.type_text();
                    if types.get(type_text).is_none() {
                        lints.push(lsp_types::Diagnostic {
                            range: lsp_types::Range {
                                start: lsp_types::Position {
                                    line: header.line_number as u32,
                                    character: 0,
                                },
                                end: lsp_types::Position {
                                    line: header.line_number as u32,
                                    character: type_text.chars().count() as u32,
                                },
                            },
                            message: format!(
                                "Type {:?} is not in ({}).",
                                type_text,
                                types
                                    .keys()
                                    .map(|t| t.to_owned())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            ..Default::default()
                        });
                    }
                });
                lints
            }),
        );

        cfg.scopes = json.scopes.unwrap_or_default();
        if !cfg.scopes.is_empty() {
            cfg.enabled_lints.push("scope_enum".to_string());
            cfg.severity.insert(
                "scope_enum".to_string(),
                lsp_types::DiagnosticSeverity::ERROR,
            );
            cfg.tests.insert(
                "scope_enum".to_string(),
                Arc::new(move |doc| -> Vec<lsp_types::Diagnostic> {
                    let mut lints: Vec<lsp_types::Diagnostic> = vec![];
                    lints.extend(
                        doc.subject
                            .as_ref()
                            .map(|header| {
                                if scopes.contains_key(header.scope_text()) {
                                    None
                                } else {
                                    let start = header.type_text().chars().count();
                                    Some(lsp_types::Diagnostic {
                                        range: lsp_types::Range {
                                            start: lsp_types::Position {
                                                line: header.line_number as u32,
                                                character: start as u32,
                                            },
                                            end: lsp_types::Position {
                                                line: header.line_number as u32,
                                                character: (start
                                                    + header.scope_text().chars().count())
                                                    as u32,
                                            },
                                        },
                                        code: Some(lsp_types::NumberOrString::String(
                                            "scope_enum".to_string(),
                                        )),
                                        source: Some(
                                            base::document::lints::LINT_PROVIDER.to_string(),
                                        ),
                                        message: format!(
                                            "Scope {:?} is not in ({}).",
                                            header.scope_text(),
                                            scopes
                                                .keys()
                                                .map(|t| t.to_owned())
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        ),
                                        ..Default::default()
                                    })
                                }
                            })
                            .flatten(),
                    );
                    lints
                }),
            );
        }

        macro_rules! handle_builtin_length_rule {
            ($code:expr, $id:ident, $f:ident, $cutoff:literal) => {
                if let Some(rule) = &json.$id {
                    let code = $code;
                    let cutoff = rule.max_length.unwrap_or($cutoff);
                    cfg.tests
                        .insert(code.to_string(), Arc::new(move |doc| $f(doc, code, cutoff)));
                    let severity: Option<lsp_types::DiagnosticSeverity> = rule
                        .severity
                        .as_ref()
                        .map(|s| -> Option<lsp_types::DiagnosticSeverity> { s.to_owned().into() })
                        .flatten();
                    if let Some(severity) = severity.map(|s| s.into()).flatten() {
                        cfg.severity.insert(code.to_string(), severity.to_owned());
                    }
                }
            };
        }
        handle_builtin_length_rule!(
            lints::HEADER_MAX_LINE_LENGTH,
            header_line_max_length,
            check_subject_line_length,
            50
        );

        handle_builtin_length_rule!(
            "body_line_max_length",
            body_line_max_length,
            check_body_line_length,
            100
        );
        // body_max_length

        macro_rules! insert_builtin {
            ($code:expr => $f:expr) => {
                cfg.tests
                    .insert($code.to_string(), Arc::new(move |doc| $f(doc, $code)));
            };
        }
        macro_rules! insert_optional_builtin {
            ($id:ident, $code:expr, $f:expr) => {
                let severity = json
                    .$id
                    .map(|rule| rule.severity).unwrap_or(Severity::None);
                if let Some(severity) = severity.clone().into() {
                    log_debug!("inserting optional builtin lint: {}", $code);
                    cfg.severity.insert($code.to_string(), severity);
                    cfg.enabled_lints.push($code.to_string());
                    insert_builtin!($code => $f);
                } else {
                    log_debug!("not inserting optional builtin lint {:?} since it was {:?} in {:?}", $code, severity, _file);
                }
            };
        }
        insert_builtin!(lints::BODY_LEADING_BLANK => lints::check_body_leading_blank);
        insert_builtin!(lints::FOOTER_LEADING_BLANK => lints::check_footer_leading_blank);
        insert_builtin!(lints::SUBJECT_EMPTY => lints::check_subject_empty);
        insert_builtin!(lints::SUBJECT_LEADING_SPACE => lints::check_subject_leading_space);
        insert_optional_builtin!(
            signed_off_by,
            crate::lints::MISSING_DCO,
            crate::lints::missing_dco
        );
        insert_optional_builtin!(
            missing_body,
            crate::lints::MISSING_BODY,
            crate::lints::missing_body
        );
        // insert_builtin!(lints::TYPE_ENUM)
        // TODO: type_enum, scope_enum
        // handle built-in boolean lints
        macro_rules! insert_severity {
            ($code:expr, $id:ident) => {
                cfg.severity.insert(
                    $code.to_string(),
                    json.$id
                        .map(|s| s.severity)
                        .map(|s| s.into())
                        .flatten()
                        .unwrap_or_else(|| {
                            lints::DEFAULT_LINT_SEVERITY.get($code).unwrap().to_owned()
                        }),
                );
            };
        }
        insert_severity!(lints::BODY_LEADING_BLANK, body_leading_blank);
        insert_severity!(lints::FOOTER_LEADING_BLANK, footer_leading_blank);
        insert_severity!(lints::SUBJECT_EMPTY, subject_empty);
        insert_severity!(lints::SUBJECT_LEADING_SPACE, missing_subject_leading_space);
        // TODO: handle plugins
        log_debug!("enabled_lints: {:?}", cfg.enabled_lints);
        Ok(cfg)
    }
}

impl base::document::lints::LintConfig for Config {
    fn enabled_lint_codes(&self) -> Vec<&str> {
        self.enabled_lints.iter().map(|s| s.as_str()).collect()
    }
    fn worktree_root(&self) -> Option<PathBuf> {
        Some(self.worktree_root.clone())
    }

    fn get_test(&self, code: &str) -> Option<&std::sync::Arc<base::document::lints::LintFn>> {
        self.tests.get(code)
    }
}

impl base::config::Config for Config {
    fn type_suggestions(&self) -> Vec<(String, String)> {
        self.types
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
    }
    fn scope_suggestions(&self) -> Vec<(String, String)> {
        // TODO: sort by recency of use on the affected files
        self.scopes
            .iter()
            .map(|(scope, doc)| (scope.to_owned(), doc.to_owned()))
            .collect()
    }
}
