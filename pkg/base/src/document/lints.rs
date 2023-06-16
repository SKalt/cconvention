use std::collections::HashMap;

pub const LINT_PROVIDER: &str = "git conventional commit language server";
lazy_static! {
static ref LINT_CODES: HashMap<&'static str, lsp_types::DiagnosticSeverity> = {
        use lsp_types::DiagnosticSeverity as Severity;
        HashMap::from([
            ("INVALID", Severity::ERROR),
            ("header-length", Severity::ERROR),
            // body-leading-blank
            // footer-leading-blank
            // footer-empty
            // header-max-length
            // scope-empty
            // type-empty
            // subject-empty

        ])
    };
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
// pub(crate) fn make_line_diagnostic(
//     line_number: usize,
//     start: u32,
//     end: u32,
//     code: &str,
//     severity: lsp_types::DiagnosticSeverity,
//     message: String,
// ) -> lsp_types::Diagnostic {
//     make_diagnostic(
//         line_number,
//         start,
//         line_number,
//         end,
//         code,
//         severity,
//         message,
//     )
// }

pub trait LintConfig {
    fn lint_severity(&self, lint_code: &str) -> lsp_types::DiagnosticSeverity {
        LINT_CODES
            .get(lint_code)
            .unwrap_or(&lsp_types::DiagnosticSeverity::WARNING)
            .to_owned()
    }

    fn make_lint(
        &self,
        code: &str,
        message: String,
        start_line: usize,
        start_char: u32,
        end_line: usize,
        end_char: u32,
    ) -> lsp_types::Diagnostic {
        make_diagnostic(
            start_line,
            start_char,
            end_line,
            end_char,
            code,
            self.lint_severity(code),
            message,
        )
    }
    fn make_line_lint(
        &self,
        code: &str,
        message: String,
        line: usize,
        start_char: u32,
        end_char: u32,
    ) -> lsp_types::Diagnostic {
        make_diagnostic(
            line,
            start_char,
            line,
            end_char,
            code,
            self.lint_severity(code),
            message,
        )
    }

    /// 0 means no limit
    fn max_subject_line_length(&self) -> u8 {
        50
    }
}
