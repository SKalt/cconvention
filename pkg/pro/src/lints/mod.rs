// © Steven Kalt
// SPDX-License-Identifier: Polyform-Noncommercial-1.0.0 OR LicenseRef-PolyForm-Free-Trial-1.0.0
use base::document::GitCommitDocument;
use base::LANGUAGE;
use lazy_static::lazy_static;
lazy_static! {
    static ref BODY_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(&LANGUAGE, include_str!("./queries/body.scm")).unwrap();
    static ref DCO_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(&LANGUAGE, include_str!("./queries/dco.scm")).unwrap();
}
pub(crate) const MISSING_BODY: &str = "missing_body";
pub(crate) const MISSING_DCO: &str = "missing_dco";
pub(crate) const MISSING_SCOPE: &str = "missing_scope";
pub(crate) fn missing_body(doc: &GitCommitDocument, code: &str) -> Vec<lsp_types::Diagnostic> {
    base::document::linting::utils::query_lint(
        doc,
        &BODY_QUERY,
        code,
        "Missing required commit body.",
    )
}
pub(crate) fn missing_dco(doc: &GitCommitDocument, code: &str) -> Vec<lsp_types::Diagnostic> {
    base::document::linting::utils::query_lint(
        doc,
        &DCO_QUERY,
        code,
        "Missing required `Signed-off-by` trailer.",
    )
}

pub fn check_scope_present(doc: &GitCommitDocument, code: &str) -> Vec<lsp_types::Diagnostic> {
    let mut lints = vec![];
    if let Some(subject) = &doc.subject {
        let scope_text = subject.scope_text();
        if scope_text.len() <= 2 || scope_text[1..scope_text.len() - 2].trim().is_empty() {
            let type_end = subject.type_text().chars().count();
            let mut lint = base::document::linting::utils::make_line_diagnostic(
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
