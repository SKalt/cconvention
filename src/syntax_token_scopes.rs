/// Language Server Protocol doesn't provide syntax highlighting, but it does
/// provide a "Semantic Tokens" API that can be used to provide syntax highlighting.
use std::{collections::HashMap, error::Error};

use lsp_types::SemanticToken;

use crate::HIGHLIGHTS_QUERY;

lazy_static! {
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
    syntax_tree: &crate::SyntaxTree,
    _params: lsp_types::SemanticTokensParams,
) -> Result<Vec<SemanticToken>, Box<dyn Error + Send + Sync>> {
    // eprintln!("params: {:?}", params);
    // https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_semanticTokens
    let mut cursor = tree_sitter::QueryCursor::new();
    let bytes = syntax_tree.code.as_bytes();
    let matches = cursor.matches(&HIGHLIGHTS_QUERY, syntax_tree.tree.root_node(), bytes);
    let names = HIGHLIGHTS_QUERY.capture_names();
    // HIGHLIGHTS_QUERY.capture_quantifiers(index)
    let mut tokens: Vec<lsp_types::SemanticToken> = Vec::new();
    let mut line: u32 = 0;
    let mut start: u32 = 0;
    for m in matches {
        for c in m.captures {
            let capture_name = names[c.index as usize].as_str();
            // TODO: handle if the client doesn't support overlapping tokens
            match capture_name {
                "text.title" | "comment" => continue, // these can overlap with other tokens
                _ => {}
            };
            // let text = c.node.utf8_text(bytes).unwrap();
            let range = c.node.range();
            let start_line: u32 = range.start_point.row.try_into().unwrap();
            if start_line > line {
                start = 0;
            };
            let delta_line: u32 = start_line - line;
            let delta_start: u32 = {
                let token_start: u32 = range.start_point.column.try_into().unwrap();
                token_start - start
            };
            line = range.end_point.row.try_into().unwrap();
            start = range.end_point.column.try_into().unwrap();

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
            // eprintln!(
            //     "capture::<{}> '{}' ({},{})-({},{}) dL{} dS{}",
            //     capture_name,
            //     text,
            //     range.start_point.row,
            //     range.start_point.column,
            //     range.end_point.row,
            //     range.end_point.column,
            //     token.delta_line,
            //     token.delta_start
            // );
        }
    }
    Ok(tokens)
}
