use std::{collections::HashMap, sync::Arc};

use crate::document::{
    linting::default::{
        check_body_leading_blank, check_footer_leading_blank, check_subject_empty,
        check_subject_leading_space, check_subject_line_length, BODY_LEADING_BLANK,
        FOOTER_LEADING_BLANK, HEADER_MAX_LINE_LENGTH, SUBJECT_EMPTY, SUBJECT_LEADING_SPACE,
    },
    GitCommitDocument,
};

use super::{default::LINT_PROVIDER, LintFn};

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

pub(crate) fn make_diagnostic(
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
    if required_missing {
        log_debug!("[{}] starting search for required capture", code);
    }
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
                log_debug!(
                    "[{}] found required capture at {:?}",
                    code,
                    c.node.start_position()
                );
                required_missing = false;
            }
        }
    }

    if required_missing {
        log_debug!("[{}] required capture not found, adding diagnostic", code);
        let mut lint = make_diagnostic(0, 0, 0, 0, message.to_string());
        lint.code = Some(lsp_types::NumberOrString::String(code.into()));
        lints.push(lint);
    }

    lints
}
