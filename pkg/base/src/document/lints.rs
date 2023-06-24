use std::collections::HashMap;

use super::GitCommitDocument;

pub const LINT_PROVIDER: &str = "git conventional commit language server";
// const lint codes
/// a fatal parse error according to the conventional commit spec
pub const INVALID: &str = "INVALID";
/// https://commitlint.js.org/#/reference-rules?id=header-length
/// note that the header is the first non-comment, non-blank line
pub const BODY_LEADING_BLANK: &str = "body-leading-blank";
pub const FOOTER_LEADING_BLANK: &str = "footer-leading-blank";
pub const HEADER_MAX_LENGTH: &str = "header-max-length";
pub const SCOPE_EMPTY: &str = "scope-empty";
pub const SUBJECT_EMPTY: &str = "subject-empty";
pub const SUBJECT_LEADING_SPACE: &str = "subject-leading-space";
use crate::LANGUAGE;

const DEFAULT_LINTS: &[&str] = &[
    BODY_LEADING_BLANK,
    FOOTER_LEADING_BLANK,
    HEADER_MAX_LENGTH,
    SCOPE_EMPTY,
    SUBJECT_EMPTY,
    SUBJECT_LEADING_SPACE,
];

lazy_static! {
    pub static ref DEFAULT_LINT_CODES: HashMap<&'static str, lsp_types::DiagnosticSeverity> = {
        use lsp_types::DiagnosticSeverity as Severity;
        HashMap::from([
            // rule of thumb: if it's in the spec and we can't auto-fix it, it's an error
            // else, it's a warning
            (INVALID, Severity::ERROR),
            (HEADER_MAX_LENGTH, Severity::WARNING), // not in the spec
            (BODY_LEADING_BLANK, Severity::WARNING), // fixable
            (FOOTER_LEADING_BLANK, Severity::WARNING), // fixable
            (SUBJECT_LEADING_SPACE, Severity::WARNING), // fixable
            (SCOPE_EMPTY, Severity::ERROR), // not fixable, probably unintentional
            (SUBJECT_EMPTY, Severity::ERROR),
        ])
    };

    static ref BAD_TRAILER_QUERY: tree_sitter::Query = tree_sitter::Query::new(
        LANGUAGE.clone(),
        "(trailer (token) @token (value)? @value)",
    ).unwrap();

    // static ref Thing: HashMap<&'static str, Arc<dyn Fn()>> = HashMap::new();
}

// IDEA: curry LintFns (code)(severity)(doc) -> Vec<_>
// currying would keep the final signature stable (doc) -> Vec<lint>
// while allowing for additional config

/// a lint-fn is a test that can return zero to many logically equivalent diagnostics
/// differentiated by a message: e.g. `[line-too-long, line-too-short]`
type LintFn<'cfg> = dyn Fn(&GitCommitDocument) -> Vec<lsp_types::Diagnostic> + 'cfg;

/// check there is exactly 1 line between the header and body
pub fn check_body_leading_blank(
    doc: &GitCommitDocument,
    code: &str,
    severity: lsp_types::DiagnosticSeverity,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    let mut body_lines = doc.get_body();
    if let Some((padding_line_number, next_line)) = body_lines.next() {
        if next_line.chars().next().is_some() {
            lints.push(make_line_diagnostic(
                code,
                severity.to_owned(),
                "there should be a blank line between the subject and the body".into(),
                padding_line_number,
                0,
                0,
            ));
        } else {
            // the first body line is blank
            // check for multiple blank lines before the body
            let mut n_blank_lines = 1u8;
            for (line_number, line) in body_lines {
                if line.chars().next().is_some() && line.chars().any(|c| !c.is_whitespace()) {
                    if n_blank_lines > 1 {
                        lints.push(make_diagnostic(
                            padding_line_number,
                            0,
                            line_number,
                            // since lsp_types::Position line numbers are 1-indexed
                            // and enumeration line numbers are 0-indexed, `first_body_line_number`
                            // is the line number of the preceding blank line
                            0,
                            code, // TODO: make distinct code? Parametrize?
                            severity.to_owned(),
                            format!("{n_blank_lines} blank lines between subject and body"),
                        ));
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

/// FIXME! port
// pub fn check_subject_line_length(
//     doc: &GitCommitDocument,
//     code: &str,
//     severity: lsp_types::DiagnosticSeverity,
// ) -> Vec<lsp_types::Diagnostic> {
//     let mut lints = vec![];
//     if let Some(subject) = doc.subject {
//         let n_chars = subject.line.chars().count();
//         let cutoff = config.max_subject_line_length();
//         if n_chars > cutoff as usize && cutoff > 0 {
//             lints.push(make_diagnostic(
//                 cutoff as u32,
//                 n_chars.try_into().unwrap(),
//                 config,
//                 format!("Header line is longer than {} characters.", cutoff),
//                 HEADER_MAX_LENGTH,
//             ))
//         }
//     }
//     lints
// }

/// Check that there's at least one leading blank before the trailers
fn check_footer_leading_blank(
    doc: &GitCommitDocument,
    code: &str,
    severity: lsp_types::DiagnosticSeverity,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(missing_line) = doc.get_missing_trailer_padding_line() {
        lints.push(make_line_diagnostic(
            code,
            severity.to_owned(),
            "Missing blank line before trailers.".into(),
            missing_line,
            0,
            0,
        ));
    }
    lints
}

/// transform a lint-test that accepts a code and severity into a function that only accepts a document.
fn transmogrify<'a, F>(
    f: F,
    code: &'a str,
    cfg: &'a dyn LintConfig,
) -> impl Fn(&GitCommitDocument) -> Vec<lsp_types::Diagnostic> + 'a
where
    F: Fn(&GitCommitDocument, &str, lsp_types::DiagnosticSeverity) -> Vec<lsp_types::Diagnostic>
        + 'a,
{
    let severity = cfg.lint_severity(code).to_owned();
    move |doc: &GitCommitDocument| f(doc, code, severity)
}

/// Check all trailers have both a key and a value
pub(crate) fn check_trailer_values(doc: &GitCommitDocument) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    let mut cursor = tree_sitter::QueryCursor::new();
    let text = doc.code.to_string();
    let trailers = cursor.matches(
        &BAD_TRAILER_QUERY,
        doc.syntax_tree.root_node(),
        text.as_bytes(),
    );
    for trailer_match in trailers {
        debug_assert!(trailer_match.captures.len() == 2);
        let value = trailer_match.captures[1].node;
        let value_text = value.utf8_text(&text.as_bytes()).unwrap().trim();
        if value_text.is_empty() {
            let key = trailer_match.captures[0].node;
            let key_text = key.utf8_text(text.as_bytes()).unwrap();
            let start = value.start_position();
            let end = value.end_position();
            lints.push(make_diagnostic(
                start.row,
                start.column as u32,
                end.row,
                end.column as u32,
                INVALID,
                lsp_types::DiagnosticSeverity::ERROR,
                format!("Empty value for trailer {:?}", key_text),
            ));
        }
    }
    lints
}

fn check_scope_present(
    doc: &GitCommitDocument,
    code: &str,
    severity: lsp_types::DiagnosticSeverity,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(subject) = &doc.subject {
        if subject.scope_text().len() == 0 {
            let type_end = subject.type_text().chars().count();
            lints.push(make_line_diagnostic(
                code,
                severity,
                "Missing scope".into(),
                subject.line_number.into(),
                type_end as u32,
                type_end as u32,
            ));
        }
    }
    lints
}

fn check_subject_leading_space(
    doc: &GitCommitDocument,
    code: &str,
    severity: lsp_types::DiagnosticSeverity,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(subject) = &doc.subject {
        let message = subject.message_text();
        let n_whitespace = message.chars().take_while(|c| c.is_whitespace()).count();
        if n_whitespace != 1 || message.chars().next() != Some(' ') {
            let start = subject.prefix_text().chars().count() as u32;
            lints.push(make_line_diagnostic(
                code,
                severity,
                "message should start with 1 space".into(),
                subject.line_number as usize,
                start,
                start + n_whitespace as u32,
            ))
            //
        }
    }
    lints
}

fn check_subject_empty(
    doc: &GitCommitDocument,
    code: &str,
    severity: lsp_types::DiagnosticSeverity,
) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(subject) = &doc.subject {
        let message = subject.message_text();
        if message.len() == 0 || message.chars().all(|c| c.is_whitespace()) {
            let start = subject.prefix_text().chars().count();
            lints.push(make_line_diagnostic(
                code,
                severity,
                "empty subject message".into(),
                subject.line_number as usize,
                start as u32,
                (start + message.chars().count()) as u32,
            ));
        }
    }
    lints
}

// TODO: curry LintFn s.t. (code)(severity)(doc) -> lint?
// FIXME: don't construct the test-map on every lint
fn construct_default_lint_tests_map<'cfg>(
    cfg: &'cfg dyn LintConfig,
) -> HashMap<&str, Box<LintFn<'cfg>>> {
    // I have to do this since HashMap::<K,V>::from<[(K, V)]> complains about `Box`ed fns
    let mut h: HashMap<&str, Box<LintFn>> = HashMap::with_capacity(2);
    macro_rules! insert {
        ($id:ident, $f:ident) => {
            h.insert($id, Box::new(transmogrify($f, $id, cfg)));
        };
    }
    h.insert(
        HEADER_MAX_LENGTH,
        Box::new(move |doc| {
            let mut lints = vec![];
            if let Some(subject) = &doc.subject {
                let cutoff = cfg.max_subject_line_length();
                let len = subject.line.chars().count();
                if len > cutoff as usize {
                    lints.push(make_line_diagnostic(
                        HEADER_MAX_LENGTH,
                        cfg.lint_severity(HEADER_MAX_LENGTH).to_owned(),
                        format!("Header line is longer than {} characters.", cutoff),
                        subject.line_number.into(),
                        cutoff as u32,
                        len as u32,
                    ))
                }
            }
            lints
        }),
    );
    insert!(BODY_LEADING_BLANK, check_body_leading_blank);
    insert!(FOOTER_LEADING_BLANK, check_footer_leading_blank);
    // TODO: check there's exactly `n` leading blank lines before trailers?
    insert!(SCOPE_EMPTY, check_scope_present);
    insert!(SUBJECT_EMPTY, check_subject_empty);
    insert!(SUBJECT_LEADING_SPACE, check_subject_leading_space);
    h
}

fn make_diagnostic(
    start_line: usize,
    start_char: u32,
    end_line: usize,
    end_char: u32,
    code: &str,
    severity: lsp_types::DiagnosticSeverity,
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
        code: Some(lsp_types::NumberOrString::String(code.into())),
        severity: Some(severity),
        message,
        ..Default::default()
    }
}

/// make a diagnostic for a single line
pub(crate) fn make_line_diagnostic(
    code: &str,
    severity: lsp_types::DiagnosticSeverity,
    message: String,
    line_number: usize,
    start: u32,
    end: u32,
) -> lsp_types::Diagnostic {
    make_diagnostic(
        line_number,
        start,
        line_number,
        end,
        code,
        severity,
        message,
    )
}

pub trait LintConfig {
    fn enabled_lint_codes(&self) -> &[&str] {
        DEFAULT_LINTS
    }
    fn lint_severity(&self, lint_code: &str) -> &lsp_types::DiagnosticSeverity {
        DEFAULT_LINT_CODES
            .get(lint_code)
            .unwrap_or(&lsp_types::DiagnosticSeverity::WARNING)
    }

    fn lint(&self, doc: &GitCommitDocument) -> Vec<lsp_types::Diagnostic>
    where
        Self: Sized,
    {
        let mut lints = vec![];
        lints.extend(doc.get_mandatory_lints());
        let code_map = construct_default_lint_tests_map(self);
        for lint_collection in self.enabled_lint_codes().iter().filter_map(|code: &&str| {
            code_map.get(*code).map(|f| f(doc)).or_else(move || {
                log_debug!("Missing lint {:?}", *code);
                None
            })
        }) {
            lints.extend(lint_collection)
        }
        lints
    }
    /// 0 means no limit
    fn max_subject_line_length(&self) -> u8 {
        50 // from https://git-scm.com/docs/git-commit#_discussion
    }
    // /// 0 means no limit
    // fn max_body_line_length(&self) -> u8 {
    //     0
    // }
}
