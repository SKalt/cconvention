pub(crate) trait Config {
    fn source(&self) -> &str; // TODO: change to PathBuf or Url
    fn types(&self) -> Vec<(&str, &str)>;
    fn scopes(&self) -> Vec<(&str, &str)>;
}
pub(crate) fn as_completion(items: &[(&str, &str)]) -> Vec<lsp_types::CompletionItem> {
    let mut result = Vec::with_capacity(items.len());
    for (label, detail) in items {
        let mut item = lsp_types::CompletionItem::new_simple(label.to_string(), detail.to_string());
        item.kind = Some(lsp_types::CompletionItemKind::ENUM_MEMBER);
        result.push(item);
    }
    result
}

pub(crate) struct DefaultConfig;
// TODO: detected scopes
impl DefaultConfig {
    pub fn new() -> Self {
        DefaultConfig
    }
}
impl Config for DefaultConfig {
    fn source(&self) -> &str {
        "<default>"
    }
    fn types(&self) -> Vec<(&str, &str)> {
        vec![
            ("feat", "adds a new feature"),
            ("fix", "fixes a bug"),
            ("docs", "changes only the documentation"),
            (
                "style",
                "changes the style but not the meaning of the code (such as formatting)",
            ),
            ("perf", "improves performance"),
            ("test", "adds or corrects tests"),
            ("build", "changes the build system or external dependencies"),
            ("chore", "changes outside the code, docs, or tests"),
            ("ci", "changes to the Continuous Integration (CI) system"),
            ("refactor", "changes the code without changing behavior"),
            ("revert", "reverts prior changes"),
        ]
    }
    fn scopes(&self) -> Vec<(&str, &str)> {
        vec![]
    }
}
