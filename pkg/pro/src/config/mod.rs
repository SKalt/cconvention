// © Steven Kalt
// SPDX-License-Identifier: Polyform-Noncommercial-1.0.0 OR LicenseRef-PolyForm-Free-Trial-1.0.0
use base::{
    document::{
        linting::{
            default::{check_body_line_length, check_subject_line_length},
            utils::make_line_diagnostic,
        },
        GitCommitDocument,
    },
    log_debug, LANGUAGE,
};
use indexmap::IndexMap;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
// TODO: move json_ish behind a feature flag
pub(crate) mod json_ish;

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Severity {
    Error,
    #[default]
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

#[derive(Default)]
pub struct Config {
    worktree_root: PathBuf,
    // TODO: source
    types: IndexMap<String, String>,
    scopes: IndexMap<String, String>,
    severity: HashMap<String, lsp_types::DiagnosticSeverity>,
    enabled_lints: Vec<String>,
    // queries: HashMap<String, tree_sitter::Query>,
    tests: HashMap<String, Arc<dyn Fn(&GitCommitDocument) -> Vec<lsp_types::Diagnostic>>>,
}

const SCOPE_ENUM: &str = "scope_enum";
const MAX_BODY_LINE_LENGTH: u16 = 100;

impl Config {
    /// Load a config from the given worktree directory, adding default types, lints, & lint severity.
    pub fn new(worktree_root: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use base::document::linting;
        // IDEA: draw lint-fn closures from a long-lived default store
        let (json, src) = json_ish::get_config(worktree_root)?
            .map(|(json, file)| (json, file.as_os_str().to_string_lossy().to_string()))
            .unwrap_or((json_ish::JsonConfig::default(), "default".to_string()));
        let enabled_lints: Vec<String> = linting::default::ENABLED_LINTS
            .iter()
            .chain(&["body_line_max_length"])
            .map(|lint_code| lint_code.to_string())
            .collect();
        let types_are_missing = json.types.as_ref().map(|t| t.is_empty()).unwrap_or(true);

        let types = if types_are_missing {
            base::config::DEFAULT_TYPES
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        } else {
            json.types.clone().unwrap()
        };
        let scopes = json.scopes.unwrap_or_default();
        let mut cfg = Config {
            worktree_root: worktree_root.to_path_buf(),
            enabled_lints,
            types: types.clone(), // TODO: figure out how to re-use cfg.types in enum-checking lint-fn
            scopes: scopes.clone(), // TODO: figure out how to re-use cfg.scopes in enum-checking lint-fn
            severity: HashMap::with_capacity(2),
            tests: HashMap::new(),
        };
        cfg.severity.insert(
            linting::default::TYPE_ENUM.to_string(),
            if types_are_missing {
                lsp_types::DiagnosticSeverity::HINT
            } else {
                lsp_types::DiagnosticSeverity::ERROR
            },
        );
        cfg.tests.insert(
            linting::default::TYPE_ENUM.to_string(),
            Arc::new(move |doc: &GitCommitDocument| {
                let mut lints = vec![];
                if let Some(header) = doc.subject.as_ref() {
                    let type_text = header.type_text();
                    if types.get(type_text).is_none() {
                        let mut lint = make_line_diagnostic(
                            format!(
                                "Type {:?} is not in ({}).",
                                type_text,
                                types
                                    .keys()
                                    .map(|t| t.to_owned())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            header.line_number as usize,
                            0,
                            type_text.chars().count() as u32,
                        );
                        lint.code = Some(lsp_types::NumberOrString::String(
                            linting::default::TYPE_ENUM.to_string(),
                        ));
                        lints.push(lint);
                    }
                };
                lints
            }),
        );

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
                    lints.extend(doc.subject.as_ref().and_then(|header| {
                        let scope_text = header.scope_text();
                        if scopes.contains_key(scope_text) {
                            None
                        } else {
                            let start = header.type_text().chars().count();
                            let end = start + scope_text.chars().count();
                            let mut lint = make_line_diagnostic(
                                format!(
                                    "Scope {:?} is not in ({}).",
                                    scope_text,
                                    scopes
                                        .keys()
                                        .map(|t| t.to_owned())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ),
                                header.line_number as usize,
                                start as u32,
                                end as u32,
                            );
                            lint.code =
                                Some(lsp_types::NumberOrString::String(SCOPE_ENUM.to_string()));
                            Some(lint)
                        }
                    }));
                    lints
                }),
            );
        }

        macro_rules! handle_builtin_length_rule {
            ($code:expr, $id:ident, $f:ident, $cutoff:expr) => {
                if let Some(rule) = json.$id {
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
            linting::default::HEADER_MAX_LINE_LENGTH,
            header_line_max_length,
            check_subject_line_length,
            base::document::linting::default::MAX_HEADER_LINE_LENGTH as u16
        );

        handle_builtin_length_rule!(
            "body_line_max_length",
            body_line_max_length,
            check_body_line_length,
            MAX_BODY_LINE_LENGTH
        );

        macro_rules! insert_builtin {
            ($code:expr => $f:expr) => {
                cfg.tests
                    .insert($code.to_string(), Arc::new(move |doc| $f(doc, $code)));
            };
        }
        macro_rules! insert_optional_builtin {
                ($id:ident, $code:expr, $f:expr) => {
                    let severity = json.$id.map(|rule| rule.severity).unwrap_or(Severity::None);
                    if let Some(severity) = severity.clone().into() {
                        log_debug!("inserting optional builtin lint: {}", $code);
                        cfg.severity.insert($code.to_string(), severity);
                        cfg.enabled_lints.push($code.to_string());
                        insert_builtin!($code => $f);
                    } else {
                        log_debug!("not inserting optional builtin lint {:?} since it was {:?} in {:?}", $code, severity, &src);
                    }
                };
            }
        insert_builtin!(linting::default::BODY_LEADING_BLANK => linting::default::check_body_leading_blank);
        insert_builtin!(linting::default::FOOTER_LEADING_BLANK => linting::default::check_footer_leading_blank);
        insert_builtin!(linting::default::SUBJECT_EMPTY => linting::default::check_subject_empty);
        insert_builtin!(linting::default::SUBJECT_LEADING_SPACE => linting::default::check_subject_leading_space);
        insert_optional_builtin!(
            missing_scope,
            crate::lints::MISSING_SCOPE,
            crate::lints::check_scope_present
        );
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
                        .map(|s| -> Option<lsp_types::DiagnosticSeverity> { s.into() })
                        .flatten()
                        .unwrap_or_else(|| {
                            linting::default::LINT_SEVERITY
                                .get($code)
                                .unwrap()
                                .to_owned()
                        }),
                );
            };
        }
        insert_severity!(linting::default::BODY_LEADING_BLANK, body_leading_blank);
        insert_severity!(linting::default::FOOTER_LEADING_BLANK, footer_leading_blank);
        insert_severity!(linting::default::SUBJECT_EMPTY, subject_empty);
        insert_severity!(
            linting::default::SUBJECT_LEADING_SPACE,
            missing_subject_leading_space
        );

        for (code, plugin) in json.plugins {
            {
                // Note: attempts to use a cache of the compiled queries (a HashMap of query_text => Query)
                //  failed because the Arc<Fn>'s lifetime kept capturing the HashMap's lifetime,
                // requiring the HashMap to be borrowed for 'static.
                // Instead of dealing with all that, always compile the query.
                // TODO: display error messages to the user
                let query = tree_sitter::Query::new(&LANGUAGE, &plugin.query).map_err(|e| {
                    format!(
                        "{:?} error compiling tree-sitter query `{}.query` @ {} line {} column {} : {:?}",
                        e.kind,
                        code,
                        &src,
                        e.row,
                        e.column,
                        e.message
                    )
                })?;
                let code = code.clone();
                cfg.tests.insert(
                    code.clone(),
                    Arc::new(move |doc| {
                        base::document::linting::utils::query_lint(
                            doc,
                            &query,
                            &code,
                            &plugin.message,
                        )
                    }),
                );
            }

            let severity: Option<lsp_types::DiagnosticSeverity> = plugin.severity.into();
            cfg.severity.insert(
                code.clone(),
                severity.unwrap_or(lsp_types::DiagnosticSeverity::ERROR),
            );
            cfg.enabled_lints.push(code);
        }
        log_debug!("enabled_lints: {:?}", cfg.enabled_lints);

        Ok(cfg)
    }
}

impl base::document::linting::LintConfig for Config {
    fn enabled_lint_codes(&self) -> Vec<&str> {
        self.enabled_lints.iter().map(|s| s.as_str()).collect()
    }
    fn worktree_root(&self) -> Option<PathBuf> {
        Some(self.worktree_root.clone())
    }

    fn get_test(&self, code: &str) -> Option<&std::sync::Arc<base::document::linting::LintFn>> {
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
