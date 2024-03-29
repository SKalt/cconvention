/// Language Server Protocol doesn't provide syntax highlighting, but it does
/// provide a "Semantic Tokens" API that can be used to provide syntax highlighting.
// © Steven Kalt
// SPDX-License-Identifier: APACHE-2.0
use std::{collections::HashMap, error::Error};

use super::LANGUAGE;
use lsp_types::SemanticToken;

lazy_static! {
    static ref HIGHLIGHTS_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(*LANGUAGE, tree_sitter_gitcommit::HIGHLIGHTS_QUERY).unwrap();
    pub static ref SYNTAX_TOKEN_LEGEND: Vec<&'static str> = vec![
        "comment",
        "error",
        "keyword",
        "parameter",
        "punctuation.delimiter",
        "punctuation.special",
        "text",
        "text.reference",
        "text.title",
        "text.uri",
        "text.warning",
    ];
    pub static ref SYNTAX_TOKEN_SCOPES: HashMap<&'static str, u32> = {
        let mut m = HashMap::new();
        for (i, s) in SYNTAX_TOKEN_LEGEND.iter().enumerate() {
            m.insert(*s, i as u32);
        }
        m
    };
}

pub(crate) fn handle_all_tokens(
    doc: &crate::document::GitCommitDocument,
    _params: lsp_types::SemanticTokensParams,
) -> Result<Vec<SemanticToken>, Box<dyn Error + Send + Sync>> {
    // https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_semanticTokens
    let mut cursor = tree_sitter::QueryCursor::new();
    let matches = cursor.matches(
        &HIGHLIGHTS_QUERY,
        doc.syntax_tree.root_node(),
        |node: tree_sitter::Node<'_>| doc.slice_of(node).chunks().map(|s| s.as_bytes()),
    );
    // TODO: use subject line tokenization from `doc.subject.unwrap()`
    let names = HIGHLIGHTS_QUERY.capture_names();
    let mut tokens: Vec<lsp_types::SemanticToken> = Vec::new();
    let mut line: u32 = 0;
    let mut start: u32 = 0;
    for m in matches {
        for c in m.captures {
            let capture_name = names[c.index as usize].as_str();
            // TODO: handle if the client doesn't support overlapping tokens
            match capture_name {
                "text.title" | "comment" | "error" => continue, // these can overlap with other tokens
                _other => {}
            };
            let range = c.node.range();
            let start_line = range.start_point.row as u32;
            let delta_line: u32 = if start_line > line {
                start = 0;
                start_line - line
            } else {
                0
            };
            let delta_start: u32 = {
                let token_start = range.start_point.column as u32;
                if token_start == 0 {
                    0
                } else {
                    token_start - start
                }
            };
            line = range.end_point.row as u32;
            start = range.end_point.column as u32;

            let token_type: u32 = *SYNTAX_TOKEN_SCOPES.get(capture_name).unwrap();
            let token = lsp_types::SemanticToken {
                delta_line,
                delta_start,
                length: (range.end_point.column - range.start_point.column)
                    .try_into()
                    .unwrap(),
                token_type,
                token_modifiers_bitset: 0,
            };

            tokens.push(token);
        }
    }
    Ok(tokens)
}
