use std::{collections::HashMap, path::PathBuf, sync::Arc};

use super::GitCommitDocument;

pub const LINT_PROVIDER: &str = "git conventional commit language server";
// const lint codes
/// a fatal parse error according to the conventional commit spec
pub const INVALID: &str = "INVALID";
/// https://commitlint.js.org/#/reference-rules?id=header-length
/// note that the header is the first non-comment, non-blank line
pub const BODY_LEADING_BLANK: &str = "body_leading_blank";
pub const FOOTER_LEADING_BLANK: &str = "footer_leading_blank";
pub const HEADER_MAX_LINE_LENGTH: &str = "header_max_line_length";
pub const BODY_MAX_LINE_LENGTH: &str = "body_max_line_length";
pub const SCOPE_EMPTY: &str = "scope_empty";
pub const SUBJECT_EMPTY: &str = "subject_empty";
pub const SUBJECT_LEADING_SPACE: &str = "missing_subject_leading_space";
pub const TYPE_ENUM: &str = "type_enum";
use crate::LANGUAGE;

pub const DEFAULT_LINTS: &[&str] = &[
    TYPE_ENUM,
    BODY_LEADING_BLANK,
    FOOTER_LEADING_BLANK,
    HEADER_MAX_LINE_LENGTH,
    SUBJECT_EMPTY,
    SUBJECT_LEADING_SPACE,
];
const DEFAULT_MAX_LINE_LENGTH: u8 = 50; // from https://git-scm.com/docs/git-commit#_discussion

lazy_static! {
    pub static ref DEFAULT_LINT_SEVERITY: HashMap<&'static str, lsp_types::DiagnosticSeverity> = {
        use lsp_types::DiagnosticSeverity as Severity;
        HashMap::from([
            // rule of thumb: if it's in the spec and we can't auto-fix it, it's an error
            // else, it's a warning
            (INVALID, Severity::ERROR),
            (HEADER_MAX_LINE_LENGTH, Severity::WARNING), // not in the spec
            (BODY_LEADING_BLANK, Severity::WARNING), // fixable
            (FOOTER_LEADING_BLANK, Severity::WARNING), // fixable
            (SUBJECT_LEADING_SPACE, Severity::WARNING), // fixable
            (SCOPE_EMPTY, Severity::ERROR), // not fixable, probably unintentional
            (SUBJECT_EMPTY, Severity::ERROR),
        ])
    };

    static ref BAD_TRAILER_QUERY: tree_sitter::Query = tree_sitter::Query::new(
        *LANGUAGE,
        include_str!("./queries/bad_trailer.scm"),
    ).unwrap();
}

/// a lint-fn is a test that can return zero to many logically equivalent diagnostics
/// differentiated by a message: e.g. `[line-too-long, line-too-short]`
pub type LintFn<'cfg> = dyn Fn(&GitCommitDocument) -> Vec<lsp_types::Diagnostic> + 'cfg;

/// check there is exactly 1 line between the header and body
pub fn check_body_leading_blank(doc: &GitCommitDocument, code: &str) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    let mut body_lines = doc.get_body();
    if let Some((padding_line_number, next_line)) = body_lines.next() {
        if next_line.chars().next().is_some() {
            let mut lint = make_line_diagnostic(
                "there should be a blank line between the subject and the body".into(),
                padding_line_number,
                0,
                0,
            );
            lint.code = Some(lsp_types::NumberOrString::String(code.into()));
            lints.push(lint);
        } else {
            // the first body line is blank
            // check for multiple blank lines before the body
            let mut n_blank_lines = 1u8;
            for (line_number, line) in body_lines {
                if line.chars().next().is_some() && line.chars().any(|c| !c.is_whitespace()) {
                    if n_blank_lines > 1 {
                        let mut lint = make_diagnostic(
                            padding_line_number,
                            0,
                            line_number,
                            // since lsp_types::Position line numbers are 1-indexed
                            // and enumeration line numbers are 0-indexed, `first_body_line_number`
                            // is the line number of the preceding blank line
                            0,
                            // code, // TODO: make distinct code? Parametrize?
                            // severity.to_owned(),
                            format!("{n_blank_lines} blank lines between subject and body"),
                        );
                        lint.code = Some(lsp_types::NumberOrString::String(code.to_string()));
                        lints.push(lint);
                    }
                    break;
                } else {
                    n_blank_lines += 1
                }
            }
            // ignore multiple trailing newlines without a body
        };
    };
    lints
}

fn check_line_length<F>(
    line: &str,
    line_number: u32,
    code: &str,
    cutoff: u16,
    message: F,
) -> Option<lsp_types::Diagnostic>
where
    F: Fn() -> String,
{
    if cutoff == 0 {
        None
    } else {
        let n_chars = line.chars().count();
        if n_chars > cutoff as usize {
            let mut lint = make_line_diagnostic(
                message(),
                line_number as usize,
                cutoff as u32,
                n_chars as u32,
            );
            lint.code = Some(lsp_types::NumberOrString::String(code.into()));
            Some(lint)
        } else {
            None
        }
    }
}
pub fn check_subject_line_length(
    doc: &GitCommitDocument,
    code: &str,
    cutoff: u16,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(subject) = &doc.subject {
        lints.extend(check_line_length(
            &subject.line,
            subject.line_number as u32,
            code,
            cutoff,
            || format!("Subject line too long (max {cutoff} chars)"),
        ));
    }
    lints
}

pub fn check_body_line_length(
    doc: &GitCommitDocument,
    code: &str,
    cutoff: u16,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    for (line_number, line) in doc.get_body() {
        lints.extend(check_line_length(
            &line.to_string(),
            line_number as u32,
            code,
            cutoff,
            || format!("Body line too long (max {cutoff} chars)"),
        ));
    }
    lints
}

/// Check that there's at least one leading blank before the trailers
pub fn check_footer_leading_blank(
    doc: &GitCommitDocument,
    code: &str,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(missing_line) = doc.get_missing_trailer_padding_line() {
        // code,
        // severity.to_owned(),
        let mut lint = make_line_diagnostic(
            "Missing blank line before trailers.".into(),
            missing_line,
            0,
            0,
        );
        lint.code = Some(lsp_types::NumberOrString::String(code.into()));
        lints.push(lint);
    }
    lints
}

pub fn query_lint(
    doc: &GitCommitDocument,
    query: &tree_sitter::Query,
    code: &str,
    message: &str,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    let mut cursor = tree_sitter::QueryCursor::new();
    let names = query.capture_names();
    let mut required_missing: bool = names.iter().any(|name| name == "required");
    // let text = doc.code.to_string();
    let matches = cursor.matches(
        query,
        doc.syntax_tree.root_node(),
        |node: tree_sitter::Node<'_>| doc.slice_of(node).chunks().map(|s| s.as_bytes()),
    );
    for m in matches {
        for c in m.captures {
            let name = &names[c.index as usize];
            if name == "forbidden" {
                let start = c.node.start_position();
                let end = c.node.end_position();
                let mut lint = make_diagnostic(
                    start.row,
                    start.column as u32,
                    end.row,
                    end.column as u32,
                    message.to_string(),
                );
                lint.code = Some(lsp_types::NumberOrString::String(code.into()));
                lints.push(lint);
            } else if name == "required" {
                required_missing = false;
            }
        }
    }

    if required_missing {
        let mut lint = make_diagnostic(0, 0, 0, 0, message.to_string());
        lint.code = Some(lsp_types::NumberOrString::String(code.into()));
        lints.push(lint);
    }

    lints
}

/// Check all trailers have both a key and a value
pub(crate) fn check_trailer_values(doc: &GitCommitDocument) -> Vec<lsp_types::Diagnostic> {
    query_lint(
        doc,
        &BAD_TRAILER_QUERY,
        "INVALID",
        "Empty value for trailer.",
    )
}

fn check_scope_present(doc: &GitCommitDocument, code: &str) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(subject) = &doc.subject {
        let len = subject.scope_text().len();
        if len > 0 && len <= 2 {
            // 2 = just the open/close parens
            let type_end = subject.type_text().chars().count();
            let mut lint = make_line_diagnostic(
                "Missing scope".into(),
                subject.line_number.into(),
                type_end as u32,
                type_end as u32,
            );
            lint.code = Some(lsp_types::NumberOrString::String(code.into()));
            lints.push(lint);
        }
    }
    lints
}

fn check_type_enum(doc: &GitCommitDocument, code: &str) -> Option<lsp_types::Diagnostic> {
    doc.subject
        .as_ref()
        .map(|header| {
            let type_text = header.type_text();
            if crate::config::DEFAULT_TYPES
                .iter()
                .any(|(t, _)| t == &type_text)
            {
                let mut lint = make_line_diagnostic(
                    format!(
                        "Type {:?} is not in ({}).",
                        type_text,
                        crate::config::DEFAULT_TYPES
                            .iter()
                            .map(|(t, _)| *t)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    header.line_number as usize,
                    0,
                    type_text.chars().count() as u32,
                );
                lint.code = Some(lsp_types::NumberOrString::String(code.into()));
                Some(lint)
            } else {
                None
            }
        })
        .flatten()
}

pub fn check_subject_leading_space(
    doc: &GitCommitDocument,
    code: &str,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(subject) = &doc.subject {
        let message = subject.message_text();
        let n_whitespace = message.chars().take_while(|c| c.is_whitespace()).count();
        if n_whitespace != 1 || !message.starts_with(' ') {
            let start = subject.prefix_text().chars().count() as u32;
            let mut lint = make_line_diagnostic(
                "message should start with 1 space".into(),
                subject.line_number as usize,
                start,
                start + n_whitespace as u32,
            );
            lint.code = Some(lsp_types::NumberOrString::String(code.into()));
            lints.push(lint);
            //
        }
    }
    lints
}

pub fn check_subject_empty(doc: &GitCommitDocument, code: &str) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(subject) = &doc.subject {
        let message = subject.message_text();
        if message.is_empty() || message.chars().all(|c| c.is_whitespace()) {
            let start = subject.prefix_text().chars().count();
            let mut lint = make_line_diagnostic(
                "empty subject message".into(),
                subject.line_number as usize,
                start as u32,
                (start + message.chars().count()) as u32,
            );
            lint.code = Some(lsp_types::NumberOrString::String(code.into()));
            lints.push(lint);
        }
    }
    lints
}

pub fn construct_default_lint_tests_map(
    cutoff: u16,
) -> HashMap<&'static str, Arc<LintFn<'static>>> {
    // I have to do this since HashMap::<K,V>::from<[(K, V)]> complains about `Arc`ed fns
    let mut tests: HashMap<&str, Arc<LintFn>> = HashMap::with_capacity(5);
    macro_rules! insert {
        ($id:ident, $f:ident) => {
            tests.insert($id, Arc::new(move |doc| $f(doc, $id)));
        };
    }
    tests.insert(
        HEADER_MAX_LINE_LENGTH,
        Arc::new(move |doc| check_subject_line_length(doc, HEADER_MAX_LINE_LENGTH, cutoff)),
    );
    insert!(BODY_LEADING_BLANK, check_body_leading_blank);
    insert!(FOOTER_LEADING_BLANK, check_footer_leading_blank);
    // TODO: check there's exactly `n` leading blank lines before trailers?
    // insert!(SCOPE_EMPTY, check_scope_present);
    insert!(SUBJECT_EMPTY, check_subject_empty);
    insert!(SUBJECT_LEADING_SPACE, check_subject_leading_space);
    tests
}

fn make_diagnostic(
    start_line: usize,
    start_char: u32,
    end_line: usize,
    end_char: u32,
    message: String,
) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        source: Some(LINT_PROVIDER.to_string()),
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: start_line as u32,
                character: start_char,
            },
            end: lsp_types::Position {
                line: end_line as u32,
                character: end_char,
            },
        },
        message,
        ..Default::default()
    }
}

/// make a diagnostic for a single line
pub(crate) fn make_line_diagnostic(
    message: String,
    line_number: usize,
    start: u32,
    end: u32,
) -> lsp_types::Diagnostic {
    make_diagnostic(line_number, start, line_number, end, message)
}

pub trait LintConfig {
    /// provides information to the user about where the lint configuration came from.
    fn source(&self) -> &str {
        "<default>"
    }
    fn worktree_root(&self) -> Option<PathBuf>;
    fn enabled_lint_codes(&self) -> &[&str] {
        DEFAULT_LINTS
    }
    fn lint_severity(&self, lint_code: &str) -> &lsp_types::DiagnosticSeverity {
        DEFAULT_LINT_SEVERITY
            .get(lint_code)
            .unwrap_or(&lsp_types::DiagnosticSeverity::WARNING)
    }

    // fn lint_tests(&self) -> &HashMap<&str, Box<LintFn>>;
    fn get_test(&self, code: &str) -> Option<&Arc<LintFn>>;
    fn lint(&self, doc: &GitCommitDocument) -> Vec<lsp_types::Diagnostic> {
        let mut diagnostics = vec![];
        diagnostics.extend(doc.get_mandatory_lints());
        // let code_map = construct_default_lint_tests_map(self);
        // for code in self.enabled_lint_codes() {}
        diagnostics.extend(
            self.enabled_lint_codes()
                .iter()
                .filter_map(|code| {
                    self.get_test(code).or_else(|| {
                        log_debug!("Missing test for code {:?}", code);
                        None
                    })
                })
                .map(|f| f(doc))
                .map(|mut v| {
                    for mut diagnostic in v.iter_mut() {
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
    /// 0 means no limit
    fn max_subject_line_length(&self) -> u8 {
        DEFAULT_MAX_LINE_LENGTH
    }
    // /// 0 means no limit
    // fn max_body_line_length(&self) -> u8 {
    //     0
    // }
}
