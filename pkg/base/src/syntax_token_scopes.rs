/// Language Server Protocol doesn't provide syntax highlighting, but it does
/// provide a "Semantic Tokens" API that can be used to provide syntax highlighting.
// © Steven Kalt
// SPDX-License-Identifier: APACHE-2.0
use std::{collections::HashMap, error::Error};

use super::LANGUAGE;
use lsp_types::SemanticToken;
use tracing::info;

lazy_static! {
    static ref HIGHLIGHTS_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(&LANGUAGE, tree_sitter_gitcommit::HIGHLIGHTS_QUERY).unwrap();
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

struct TokenCapabilities(u8);
impl TokenCapabilities {
    const MULTILINE: u8 = 0b0000_1111;
    const OVERLAP: u8 = 0b1111_0000;
    pub fn new(multiline: bool, overlap: bool) -> Self {
        Self(if multiline { Self::MULTILINE } else { 0 } | if overlap { Self::OVERLAP } else { 0 })
    }
    #[inline]
    fn supports_multiline(&self) -> bool {
        (self.0 & Self::MULTILINE) != 0
    }
    #[inline]
    fn supports_overlap(&self) -> bool {
        (self.0 & Self::OVERLAP) != 0
    }
}
pub(crate) fn handle_all_tokens(
    _client_capabilities: &lsp_types::ClientCapabilities,
    doc: &crate::document::GitCommitDocument,
    _params: lsp_types::SemanticTokensParams,
) -> Result<Vec<SemanticToken>, Box<dyn Error + Send + Sync>> {
    let _client = {
        // see https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#semanticTokensClientCapabilities
        let semantic_token_capabilities = _client_capabilities
            .text_document
            .as_ref()
            .and_then(|td| td.semantic_tokens.as_ref());
        semantic_token_capabilities
            .map(|st| {
                TokenCapabilities::new(
                    st.multiline_token_support.unwrap_or(false),
                    st.overlapping_token_support.unwrap_or(false),
                )
            })
            .unwrap_or(TokenCapabilities::new(false, false))
    };
    info!(
        "client semantic token support: multiline: {}; overlap: {}",
        _client.supports_multiline(),
        _client.supports_overlap()
    );
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
    let mut prev_seen_line: u32 = 0; // the line number of the last seen capture
    let mut start_col: u32 = 0; // the column number of the start of the token
    for m in matches {
        for capture in m.captures {
            let capture_name = names[capture.index as usize];
            // TODO: handle if the client doesn't support overlapping tokens
            match capture_name {
                "text.title" | "comment" | "error" => continue, // these can overlap with other tokens
                _ => {}
            };
            let range = capture.node.range();
            if !_client.supports_multiline() && range.start_point.row < range.end_point.row {
                continue; // since this is a multiline token
            }
            let start_line = range.start_point.row as u32;
            let delta_line: u32 = start_line - prev_seen_line;
            if start_line > prev_seen_line {
                start_col = 0;
            }
            let delta_start: u32 = {
                let token_start = range.start_point.column as u32;
                if token_start == 0 {
                    0
                } else {
                    token_start - start_col
                }
            };
            prev_seen_line = range.end_point.row as u32;
            start_col = range.end_point.column as u32;

            let token_type: u32 = *SYNTAX_TOKEN_SCOPES.get(capture_name).unwrap();
            // See https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_semanticTokens
            let token = lsp_types::SemanticToken {
                delta_line,  // token line number, relative to the previous token
                delta_start, // token start character, relative to the previous token
                length: {
                    if _client.supports_multiline() {
                        panic!("unable to calculate length of multiline token")
                    } else {
                        (range.end_point.column - range.start_point.column)
                            .try_into()
                            .unwrap() // <- panicking if the line is over 4 * 10^9
                                      // characters long is fine
                    }
                },
                token_type,
                token_modifiers_bitset: 0,
            };

            tokens.push(token);
        }
    }
    Ok(tokens)
}
