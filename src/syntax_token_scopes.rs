/// Language Server Protocol doesn't provide syntax highlighting, but it does
/// provide a "Semantic Tokens" API that can be used to provide syntax highlighting.
use std::collections::HashMap;

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

pub(crate) fn yeet() {}
