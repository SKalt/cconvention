use base::document::GitCommitDocument;
use base::LANGUAGE;
use lazy_static::lazy_static;
lazy_static! {
    static ref BODY_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(*LANGUAGE, include_str!("./queries/body.scm")).unwrap();
    static ref DCO_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(*LANGUAGE, include_str!("./queries/dco.scm")).unwrap();
}
pub(crate) const MISSING_BODY: &str = "missing_body";
pub(crate) const MISSING_DCO: &str = "missing_dco";
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
