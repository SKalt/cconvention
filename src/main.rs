use crop::{self, Rope, RopeSlice};
use lsp_server::{self, Message, Notification, RequestId, Response};
use lsp_types::DidChangeTextDocumentParams;
use lsp_types::{
    self, CodeActionParams, CompletionItem, CompletionParams, Diagnostic, DiagnosticSeverity,
    DidOpenTextDocumentParams, DocumentLinkParams, DocumentOnTypeFormattingParams,
    DocumentRangeFormattingParams, HoverParams, InitializeResult, Position, SelectionRangeParams,
    SemanticTokensLegend, ServerInfo, TextDocumentContentChangeEvent, Url,
    WillSaveTextDocumentParams,
};
use std::error::Error;

mod syntax_token_scopes;

extern crate serde_json;

#[macro_use]
extern crate lazy_static;

const LINT_PROVIDER: &str = "git conventional commit language server";
lazy_static! {
    static ref CAPABILITIES: lsp_types::ServerCapabilities = get_capabilities();
    static ref LANGUAGE: tree_sitter::Language = tree_sitter_gitcommit::language();
    static ref SUBJECT_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(LANGUAGE.clone(), "(subject) @subject",).unwrap();
    static ref DEFAULT_TYPES: Vec<lsp_types::CompletionItem> = {
        vec![
            lsp_types::CompletionItem {
                label: "feat".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("adds a new feature".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "fix".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("fixes a bug".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "docs".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("changes only the documentation".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "style".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some(
                    "changes the style but not the meaning of the code (such as formatting)"
                        .to_string(),
                ),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "perf".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("improves performance".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "test".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("adds or corrects tests".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "build".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("changes the build system or external dependencies".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "chore".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("changes outside the code, docs, or tests".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "ci".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("changes to the Continuous Integration (CI) system".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "refactor".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("changes the code without changing behavior".to_string()),
                ..Default::default()
            },
            lsp_types::CompletionItem {
                label: "revert".to_string(),
                kind: Some(lsp_types::CompletionItemKind::ENUM_MEMBER),
                detail: Some("reverts prior changes".to_string()),
                ..Default::default()
            },
        ]
    };
}

/// a constant (a function that always returns the same thing) that returns the
/// server's capabilities.  We need to wrap the constant server capabilities in a function
/// since the server's capabilities include a `Vec` which allocates memory.
fn get_capabilities() -> lsp_types::ServerCapabilities {
    lsp_types::ServerCapabilities {
        position_encoding: None, //Some(lsp_types::PositionEncodingKind::UTF8),
        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Options(
            lsp_types::TextDocumentSyncOptions {
                open_close: Some(true), // open, close notifications sent to server
                change: Some(lsp_types::TextDocumentSyncKind::INCREMENTAL),
                will_save: None,
                will_save_wait_until: None,
                save: Some(lsp_types::TextDocumentSyncSaveOptions::SaveOptions(
                    lsp_types::SaveOptions {
                        include_text: Some(true),
                    },
                )),
            },
        )),
        hover_provider: None,
        // FIXME: provide hover info about types, scopes
        // hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        // TODO: provide selection range
        // selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
        completion_provider: Some(lsp_types::CompletionOptions {
            resolve_provider: None,
            trigger_characters: None,
            all_commit_characters: None,
            work_done_progress_options: lsp_types::WorkDoneProgressOptions {
                work_done_progress: None,
            },
            completion_item: None,
        }),
        // code_action_provider: Some(lsp_types::CodeActionProviderCapability::Options(
        //     lsp_types::CodeActionOptions {
        //         code_action_kinds: Some(vec![
        //             CodeActionKind::EMPTY,
        //             CodeActionKind::QUICKFIX,
        //             CodeActionKind::REFACTOR,
        //             CodeActionKind::SOURCE_FIX_ALL,
        //         ]),
        //         work_done_progress_options: lsp_types::WorkDoneProgressOptions {
        //             work_done_progress: None,
        //         },
        //         resolve_provider: None,
        //     },
        // )),
        // https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_formatting
        document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
        // https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_rangeFormatting
        document_range_formatting_provider: None,
        // https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_onTypeFormatting
        document_on_type_formatting_provider: Some(lsp_types::DocumentOnTypeFormattingOptions {
            first_trigger_character: "\n".to_string(),
            more_trigger_character: None,
        }),

        document_link_provider: None,
        // TODO: use the tree-sitter parser to find links to affected files
        // document_link_provider: Some(lsp_types::DocumentLinkOptions {
        //     resolve_provider: Some(true),
        //     work_done_progress_options: lsp_types::WorkDoneProgressOptions {
        //         work_done_progress: None,
        //     },
        // }),
        folding_range_provider: None, // TODO: actually do this though
        // TODO: jump from type/scope -> definition in config
        // https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition
        // definition_provider: None,
        declaration_provider: None, // maybe later, for jumping to configuration
        execute_command_provider: None, // maybe later for executing code blocks
        workspace: None,            // maybe later, for git history inspection
        semantic_tokens_provider: Some(
            // provides syntax highlighting!
            lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(
                lsp_types::SemanticTokensOptions {
                    work_done_progress_options: lsp_types::WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    legend: SemanticTokensLegend {
                        token_types: syntax_token_scopes::SYNTAX_TOKEN_LEGEND
                            .iter()
                            .map(|tag| lsp_types::SemanticTokenType::new(*tag))
                            .collect(),
                        token_modifiers: vec![
                        // lsp_types::SemanticTokenModifier
                        ],
                    },
                    range: None, // TODO: injection ranges
                    full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
                },
            ),
        ),
        ..Default::default()
    }
}

#[derive(Debug, Clone, Copy)]
struct StrIndex {
    byte: u8,
    char: u8,
}

/// char indices of significant characters in a conventional commit header.
/// Used to parse the header into its constituent parts and to find relevant completions.
#[derive(Debug, Default, Clone)]
struct Subject {
    line: String,
    line_number: usize,
    // TODO: compact to u8? Subjects should never be more than 255 chars
    /// the first opening parenthesis in the subject line
    /// ```txt
    /// ### valid ###
    /// type(scope)!: subject
    /// #   ( -> <Some(4)>
    /// type!: subject
    /// <None>
    /// type: subject()
    /// # -> <None>
    /// ### invalid ###
    /// type ( scope ) ! : subject
    /// #    ( -> <Some(5)>
    /// type scope)!: subject
    /// <None>
    /// ```
    open_paren: Option<StrIndex>,
    /// the first closing parenthesis in the subject line.
    /// ```txt
    /// ### valid ###
    /// type(scope)!: subject
    /// #         ) -> <Some(9)>
    /// type!: subject
    /// # -> <None>
    /// type: subject()
    /// # -> <None>
    /// ### invalid ###
    /// type ( scope ) ! : subject
    /// #            ) -> <Some(11)>
    /// type scope)!: subject
    /// <None>
    /// ```
    close_paren: Option<StrIndex>,
    /// the first exclamation mark in the subject line *before* the colon.
    /// ```txt
    /// ### valid ###
    /// type(scope)!: subject
    /// #          ! -> <Some(10)>
    /// type!: subject
    /// #   ! -> <Some(3)>
    /// type: subject!()
    /// # -> <None>
    bang: Option<StrIndex>,
    colon: Option<StrIndex>,
    space: Option<StrIndex>,
}
impl Subject {
    /// parse a conventional commit header into its byte indices
    /// ```txt
    /// type(scope)!: subject
    ///     (     )!:_
    /// ```
    fn new(header: String, line_number: usize) -> Self {
        let mut subject = Self::default();
        subject.line_number = line_number;
        subject.line = header;
        let line = subject.line.as_str();
        // type(scope)!: subject
        // type(scope)! subject
        let prefix = if let Some(colon) = line.find(':') {
            let colon_index = StrIndex {
                byte: colon.try_into().unwrap(),
                char: line[..colon].chars().count().try_into().unwrap(),
            };
            subject.colon = Some(colon_index);
            &line[..colon]
        } else {
            line
        };

        // type(scope)!
        // type(scope) subject
        let prefix = if let Some(bang) = prefix.find('!') {
            let prefix = &prefix[..bang];
            let bang_index = StrIndex {
                byte: bang.try_into().unwrap(),
                char: prefix.chars().count().try_into().unwrap(),
            };
            subject.bang = Some(bang_index);
            prefix
        } else {
            prefix
        };

        // type(scope
        // type(scope subject
        let prefix = if let Some(close_paren) = prefix.find(')') {
            let prefix = &prefix[..close_paren];
            let close_paren_index = StrIndex {
                byte: close_paren.try_into().unwrap(),
                char: prefix.chars().count().try_into().unwrap(),
            };
            subject.close_paren = Some(close_paren_index);
            prefix
        } else {
            prefix
        };

        // type(
        // type scope subject
        // type subject
        let prefix = if let Some(open_paren) = prefix.find('(') {
            let prefix = &prefix[..open_paren];
            let open_paren_index = StrIndex {
                byte: open_paren.try_into().unwrap(),
                char: prefix.chars().count().try_into().unwrap(),
            };

            subject.open_paren = Some(open_paren_index);
            prefix
        } else {
            prefix
        };

        // type scope subject
        // type subject
        if let Some(space) = prefix.find(' ') {
            let prefix = &prefix[..space];
            let space_index = StrIndex {
                byte: space.try_into().unwrap(),
                char: prefix.chars().count().try_into().unwrap(),
            };

            subject.space = Some(space_index);
        }
        subject
    }
    /// returns the char index of the end of the type, NON-INCLUSIVE:
    /// ```txt
    /// type(scope): <subject>
    ///    ^
    /// ```
    fn type_end(&self) -> StrIndex {
        self.open_paren
            .or(self.bang)
            .or(self.colon)
            .or(self.space)
            .unwrap_or_else(|| StrIndex {
                char: self.line.chars().count().try_into().unwrap(),
                byte: self.line.len().try_into().unwrap(),
            })
    }

    fn scope_start(&self) -> Option<StrIndex> {
        if self.open_paren.is_none() && self.close_paren.is_none() {
            return None;
        } else {
            return self
                .open_paren
                .or(Some(self.type_end()))
                .map(|i| self.next_char(i));
        }
    }
    fn next_char(&self, index: StrIndex) -> StrIndex {
        if let Some(c) = &self.line.as_str()[index.byte as usize..].chars().next() {
            return StrIndex {
                byte: index.byte + TryInto::<u8>::try_into(c.len_utf8()).unwrap(),
                char: index.char + 1,
            };
        } else {
            return index;
        }
    }
    /// returns the char index of the end of the scope:
    /// ```txt
    /// type(scope): <subject>
    /// #         ^
    /// type scope): <subject>
    /// #         ^
    /// type(scope: <subject>
    /// #        ^
    /// type: <subject>
    /// # -> <None>
    /// ```
    fn scope_end(&self) -> Option<StrIndex> {
        if self.close_paren.is_none() && self.open_paren.is_none() {
            return None;
        } else {
            self.close_paren
                // .map(|i| {
                //     if let Some(c) = self.line.as_str()[i.byte as usize..].chars().next() {
                //         return StrIndex {
                //             byte: i.byte + TryInto::<u8>::try_into(c.len_utf8()).unwrap(),
                //             char: i.char + 1,
                //         };
                //     } else {
                //         return i;
                //     };
                // })
                .or(self.bang)
                .or(self.colon)
                .or(self.space)
        }
    }

    fn prefix_end(&self) -> Option<StrIndex> {
        self.colon.or(self.bang).or(self.close_paren).or(self.space)
    }

    fn debug_indices(&self) {
        // TODO: ensure this function call is a no-op in release builds
        eprintln!("debugging indices");
        let mut cursor = 0;

        eprint!("\t{}\n\t", self.line);
        if let Some(open_paren) = &self.open_paren {
            while cursor < open_paren.char {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!("(");
        }
        if let Some(close_paren) = &self.close_paren {
            while cursor < close_paren.char {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!(")");
        }
        if let Some(bang) = &self.bang {
            while cursor < bang.char {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!("!");
        }
        if let Some(colon) = &self.colon {
            while cursor < colon.char {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!(":");
        }
        if let Some(space) = &self.space {
            while cursor < space.char {
                cursor += 1;
                eprint!(" ");
            }
            eprint!("_")
        }
        eprintln!("\n");
    }
    fn debug_ranges(&self) {
        // TODO: ensure this function call is a no-op in release builds
        eprintln!("debugging ranges:");
        eprint!("\t{}\n\t", self.line);
        let mut cursor = 0u8;
        while cursor < self.type_end().char {
            cursor += 1;
            eprint!("t")
        }
        if let Some(open_paren) = &self.open_paren {
            while cursor < open_paren.char {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!("(");
        }
        while cursor < self.scope_end().map(|i| i.char).unwrap_or(0) {
            cursor += 1;
            eprint!("s");
        }
        if let Some(close_paren) = &self.close_paren {
            while cursor < close_paren.char {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!(")");
        }
        if let Some(bang) = &self.bang {
            while cursor < bang.char {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!("!")
        }
        if let Some(colon) = &self.colon {
            while cursor < colon.char {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!(":")
        }
        if let Some(space) = &self.space {
            while cursor < space.char {
                cursor += 1;
                eprint!(" ")
            }
            eprint!("_")
        }
        eprintln!("\n");
    }

    fn get_diagnostics(&self, cutoff: usize) -> Vec<Diagnostic> {
        let mut lints = Vec::new();
        let line = self.line.as_str();
        {
            // validate line length
            let n_chars = line.chars().count();
            if n_chars > cutoff {
                // validate the subject line length
                lints.push(Diagnostic {
                    range: lsp_types::Range {
                        start: Position {
                            line: self.line_number as u32,
                            character: cutoff as u32,
                        },
                        end: Position {
                            line: self.line_number as u32,
                            character: n_chars as u32,
                        },
                    },
                    severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                    code: None,
                    source: Some(LINT_PROVIDER.to_string()),
                    message: format!("subject line is > {cutoff} characters", cutoff = cutoff),
                    related_information: None,
                    tags: None,
                    data: None,
                    code_description: None,
                });
            }
        };
        {
            // lint for space in the type
            let type_ = &line[0..self.type_end().byte as usize];
            eprintln!("type: >{:?}<", type_);
            if type_.chars().any(|c| c.is_whitespace()) {
                lints.push(Diagnostic {
                    range: lsp_types::Range {
                        start: Position {
                            line: self.line_number as u32,
                            character: 0,
                        },
                        end: Position {
                            line: self.line_number as u32,
                            character: self.type_end().char as u32,
                        },
                    },
                    severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                    code: None,
                    source: Some(LINT_PROVIDER.to_string()),
                    message: format!("type contains whitespace"),
                    related_information: None,
                    tags: None,
                    data: None,
                    code_description: None,
                });
            }
        }
        {
            // lint the scope, if any
            if let Some(scope_end) = self.scope_end() {
                // there's a scope
                let scope_start = self.scope_start().unwrap();

                let scope = {
                    let start_byte = scope_start.byte as usize;
                    &line[start_byte..scope_end.byte as usize]
                };
                eprintln!("scope: >{:?}<", scope);
                if self.open_paren.is_none() {
                    lints.push(Diagnostic {
                        range: lsp_types::Range {
                            start: Position {
                                line: self.line_number as u32,
                                character: scope_start.char as u32,
                            },
                            end: Position {
                                line: self.line_number as u32,
                                character: (scope_start.char + 1) as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some(LINT_PROVIDER.to_string()),
                        message: "Missing opening parenthesis".to_string(),
                        ..Default::default()
                    })
                } else if self.close_paren.is_none() {
                    lints.push(Diagnostic {
                        range: lsp_types::Range {
                            start: Position {
                                line: self.line_number as u32,
                                character: (scope_end.char - 1) as u32,
                            },
                            end: Position {
                                line: self.line_number as u32,
                                character: (scope_end.char) as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some(LINT_PROVIDER.to_string()),
                        message: "Missing closing parenthesis".to_string(),
                        ..Default::default()
                    });
                }
                if scope.chars().any(|c| c.is_whitespace()) {
                    lints.push(Diagnostic {
                        range: lsp_types::Range {
                            start: Position {
                                line: self.line_number as u32,
                                character: self.type_end().char as u32,
                            },
                            end: Position {
                                line: self.line_number as u32,
                                character: scope_end.char as u32,
                            },
                        },
                        severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                        code: None,
                        source: Some(LINT_PROVIDER.to_string()),
                        message: format!("scope contains whitespace"),
                        related_information: None,
                        tags: None,
                        data: None,
                        code_description: None,
                    });
                }
            } else {
                // no scope
            }
        }
        {
            // check the colon is present
            if let Some(colon) = self.colon {
                let start = self
                    .scope_end()
                    .map(|i| self.next_char(i))
                    .unwrap_or_else(|| self.type_end());
                let span = &line[start.byte as usize..colon.byte as usize];
                eprintln!("span: >{:?}<", span);
                for c in span.chars() {
                    if c != '!' {
                        lints.push(Diagnostic {
                            range: lsp_types::Range {
                                start: Position {
                                    line: self.line_number as u32,
                                    character: start.char as u32,
                                },
                                end: Position {
                                    line: self.line_number as u32,
                                    character: colon.char as u32,
                                },
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            source: Some(LINT_PROVIDER.to_string()),
                            message: format!("Illegal character before colon: {:?}", c),
                            ..Default::default()
                        });
                        break;
                    }
                }
            } else {
                let start = self
                    .bang
                    .or(self.close_paren)
                    .map(|i| self.next_char(i))
                    .unwrap_or_else(|| self.type_end());
                let end = self.next_char(start);
                let span = &line[start.byte as usize..end.byte as usize];
                eprintln!("span: >{:?}<", span);

                lints.push(Diagnostic {
                    range: lsp_types::Range {
                        start: Position {
                            line: self.line_number as u32,
                            character: start.char as u32,
                        },
                        end: Position {
                            line: self.line_number as u32,
                            character: end.char as u32,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some(LINT_PROVIDER.to_string()),
                    message: "Missing colon".to_string(),
                    ..Default::default()
                });
            }
        }
        {
            // lint space after colon
            let start = self
                .colon
                .or(self.bang)
                .or(self.close_paren)
                .or_else(|| self.scope_end())
                .map(|i| self.next_char(i))
                .unwrap_or_else(|| self.type_end());
            if !self.line.as_str()[start.byte as usize..]
                .chars()
                .next()
                .map(|c| c == ' ')
                .unwrap_or(false)
            {
                let end = self.next_char(start);
                let span = &line[start.byte as usize..end.byte as usize];
                eprintln!("span: >{:?}<", span);
                lints.push(Diagnostic {
                    range: lsp_types::Range {
                        start: Position {
                            line: self.line_number as u32,
                            character: start.char as u32,
                        },
                        end: Position {
                            line: self.line_number as u32,
                            character: end.char as u32,
                        },
                    },
                    severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                    code: None,
                    source: Some(LINT_PROVIDER.to_string()),
                    message: format!("Missing space after colon"),
                    related_information: None,
                    tags: None,
                    data: None,
                    code_description: None,
                });
            }
        };
        lints
    }
}

struct GitCommitDocument {
    code: crop::Rope,
    parser: tree_sitter::Parser, // since the parser is stateful, it needs to be owned by the document
    syntax_tree: tree_sitter::Tree,
    subject: Option<Subject>,
}

/// given a line/column position in the text, return the the byte offset of the position
fn find_byte_offset(text: &Rope, pos: Position) -> usize {
    let mut byte_offset: usize = 0;
    // only do the conversions once
    let line_index = pos.line as usize;
    let char_index = pos.character as usize;
    for (i, line) in text.raw_lines().enumerate() {
        // includes line breaks
        if i < line_index {
            byte_offset += line.byte_len();
            continue;
        } else {
            let line = line.to_string();
            for (i, c) in line.chars().enumerate() {
                eprintln!("c: >{:?}<; offset: {}", c, byte_offset);
                if i >= char_index {
                    // don't include the target char in the byte-offset
                    return byte_offset;
                } else {
                    byte_offset += c.len_utf8();
                }
            }
        }
    }
    return byte_offset;
}

fn get_subject_line(code: &Rope) -> Option<(RopeSlice, usize)> {
    for (number, line) in code.lines().enumerate() {
        if line.bytes().next() != Some(b'#') {
            return Some((line, number));
        }
    }
    None
}

/// transform a line/column position into a tree-sitter Point struct
fn to_point(p: lsp_types::Position) -> tree_sitter::Point {
    tree_sitter::Point {
        row: p.line as usize,
        column: p.character as usize,
    }
}

impl GitCommitDocument {
    fn new(text: String) -> Self {
        let code = crop::Rope::from(text.clone());
        let subject = if let Some((subject, line_number)) = get_subject_line(&code) {
            Some(Subject::new(subject.to_string(), line_number))
        } else {
            None
        };
        let mut parser = {
            let language = tree_sitter_gitcommit::language();
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(language).unwrap();
            parser.set_timeout_micros(500_000); // .5 seconds
            parser
        };
        let syntax_tree = parser.parse(&text, None).unwrap();
        GitCommitDocument {
            code,
            parser,
            syntax_tree,
            subject,
        }
    }
    fn recompute_indices(&mut self) {
        self.subject = if let Some((subject, line_number)) = self._get_subject_line_with_number() {
            Some(Subject::new(subject.to_string(), line_number))
        } else {
            None
        };
    }
    fn _get_subject_line_with_number(&self) -> Option<(String, usize)> {
        if let Some(node) = self.get_ts_subject_line() {
            return Some((
                node.utf8_text(self.code.to_string().as_bytes())
                    .unwrap()
                    .to_string(),
                node.start_position().row,
            ));
        }
        if let Some((text, number)) = get_subject_line(&self.code) {
            return Some((text.to_string(), number));
        }
        None
    }
    fn get_ts_subject_line(&self) -> Option<tree_sitter::Node> {
        let mut cursor = tree_sitter::QueryCursor::new();
        let names = SUBJECT_QUERY.capture_names();
        let code = self.code.to_string();
        let matches = cursor.matches(
            &SUBJECT_QUERY,
            self.syntax_tree.root_node(),
            code.as_bytes(),
        );
        for m in matches {
            for c in m.captures {
                let name = names[c.index as usize].as_str();
                match name {
                    "subject" => {
                        return Some(c.node);
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }
        None
    }
    fn get_subject_line_number(&self) -> usize {
        if let Some(node) = self.get_ts_subject_line() {
            return node.start_position().row;
        }
        if let Some((_, number)) = get_subject_line(&self.code) {
            return number;
        }
        // TODO: handle situation where all the lines are comments
        0
    }

    fn edit(&mut self, edits: &[TextDocumentContentChangeEvent]) -> &mut Self {
        // FIXME: sometimes deletions/bulk inserts cause duplicate characters to creep in
        for edit in edits {
            debug_assert!(edit.range.is_some(), "range is none");
            if edit.range.is_none() {
                continue;
            }
            let range = edit.range.unwrap();
            let start_byte = find_byte_offset(&self.code, range.start);
            let end_byte = find_byte_offset(&self.code, range.end);

            eprintln!("start..end byte: {}..{}", start_byte, end_byte);
            self.code.replace(start_byte..end_byte, &edit.text);
            eprintln!("new code:\n{}", self.code.to_string());
            let new_end_position = match edit.text.rfind('\n') {
                Some(ind) => {
                    let num_newlines = edit.text.bytes().filter(|&c| c == b'\n').count();
                    tree_sitter::Point {
                        row: range.start.line as usize + num_newlines,
                        column: edit.text.len() - ind,
                    }
                }
                None => tree_sitter::Point {
                    row: range.end.line as usize,
                    column: range.end.character as usize + edit.text.len(),
                },
            };
            eprintln!("found end position, submitting edit");
            self.syntax_tree.edit(&tree_sitter::InputEdit {
                start_byte,
                old_end_byte: end_byte,
                new_end_byte: start_byte + edit.text.len(),
                start_position: to_point(range.start),
                old_end_position: to_point(range.end),
                new_end_position,
            });
            eprintln!("parsing");
            {
                // update the semantic ranges --------------------------------------
                let prev_tree = &self.syntax_tree;
                self.syntax_tree = self
                    .parser
                    .parse(&(self.code.to_string()), Some(prev_tree))
                    .unwrap();
                eprintln!("{}", &self.syntax_tree.root_node().to_sexp());
                // TODO: detect if the subject line changed.
                // HACK: for now, just recompute the indices
                self.recompute_indices();
            }
        }

        self
    }

    fn get_diagnostics(&self) -> Vec<Diagnostic> {
        let mut lints = if let Some(subject) = &self.subject {
            subject.get_diagnostics(50)
        } else {
            vec![]
        };
        { // validation of message body
             // TODO: if there's a body, check for a blank line after the subject
             // TODO: check trailers are grouped and trailing
        }
        { // trailer misspellings
        }

        lints
    }
}

/// a Server instance owns a `lsp_server::Connection` instance and a mutable
/// syntax tree, representing an actively edited .git/GIT_COMMIT_EDITMSG file.
struct Server {
    commit: GitCommitDocument,
    connection: lsp_server::Connection,
}

enum ServerLoopAction {
    /// Keep on servin'
    Continue,
    /// Shut down successfully
    Break,
}

/// extract the parameters from a specific kind of request
fn get_request_params<RequestMethod>(req: &lsp_server::Request) -> Result<RequestMethod::Params, ()>
where
    RequestMethod: lsp_types::request::Request,
    RequestMethod::Params: serde::de::DeserializeOwned,
{
    if req.method == RequestMethod::METHOD {
        let params = serde_json::from_value::<RequestMethod::Params>(req.params.clone()).unwrap();
        return Ok(params);
    }
    Err(())
}

/// extract the parameters from a specific kind of Notification
fn get_notification_params<NotificationKind>(
    req: &lsp_server::Notification,
) -> Result<NotificationKind::Params, ()>
where
    NotificationKind: lsp_types::notification::Notification,
    NotificationKind::Params: serde::de::DeserializeOwned,
{
    if req.method == NotificationKind::METHOD {
        let params =
            serde_json::from_value::<NotificationKind::Params>(req.params.clone()).unwrap();
        return Ok(params);
    }
    Err(())
}

// basic methods
impl Server {
    /// communicate the server's capabilities with the client
    fn init(&mut self) -> Result<&mut Self, Box<dyn Error + Send + Sync>> {
        // let capabilities = &params.capabilities;
        let (id, _) = self.connection.initialize_start()?;
        let response = InitializeResult {
            capabilities: CAPABILITIES.clone(),
            server_info: Some(ServerInfo {
                name: "conventional-commit-language-server".to_owned(),
                version: None, // TODO: send over server info based on current build
            }),
        };
        self.connection
            .initialize_finish(id, serde_json::json!(response))?;
        Ok(self)
    }

    /// create a fresh server with a stdio-based connection.
    fn from_stdio() -> Self {
        let (conn, _io) = lsp_server::Connection::stdio();
        Server {
            commit: GitCommitDocument::new("".to_owned()),
            connection: conn,
        }
    }
    fn serve(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        while let Ok(message) = self.connection.receiver.recv() {
            match self.handle_message(message)? {
                ServerLoopAction::Continue => continue,
                ServerLoopAction::Break => break,
            }
        }
        Ok(())
    }
    /// write a Response to the `connection.sender`. Works or panics.
    fn respond(&mut self, response: Response) {
        self.connection
            .sender
            .send(Message::Response(response))
            .unwrap()
    }

    fn handle_message(
        &mut self,
        message: Message,
    ) -> Result<ServerLoopAction, Box<dyn Error + Sync + Send>> {
        match message {
            Message::Request(request) => {
                // if request.method.as_str() == <lsp_types::request::Shutdown as request::Request>::METHOD {
                //     self.connection.sender.send(Message::Notification(lsp_types::notification::Exit::)).unwrap();
                //     return Ok(ServerLoopAction::Break);
                // }
                let response = self.handle_request(request)?;
                self.respond(response);
                Ok(ServerLoopAction::Continue)
            }
            Message::Response(resp) => {
                eprintln!("response: {:?}", resp);
                Ok(ServerLoopAction::Continue)
            }
            Message::Notification(notification) => self.handle_notification(notification),
        }
    }
}

// notification handlers
impl Server {
    fn handle_notification(
        &mut self,
        notification: lsp_server::Notification,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        use lsp_types::notification::*;
        macro_rules! handle {
            ($method:ty => $handler:ident) => {
                if let Ok(params) = get_notification_params::<$method>(&notification) {
                    return Server::$handler(self, params);
                }
            };
        }
        eprintln!("notification: {:?}", notification);
        handle!(DidChangeTextDocument => handle_did_change);
        handle!(DidOpenTextDocument => handle_open);
        handle!(Exit => handle_exit);
        // DidChangeConfiguration
        // WillSaveTextDocument
        handle!(DidCloseTextDocument => handle_close);
        handle!(DidSaveTextDocument => handle_save);
        // DidChangeWatchedFiles
        // WorkDoneProgressCancel

        Ok(ServerLoopAction::Continue)
    }
    fn publish_diagnostics(&self, uri: Url, diagnostics: Vec<lsp_types::Diagnostic>) {
        eprintln!("publishing diagnostics: {:?}", diagnostics);
        let params = lsp_types::PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        };
        self.connection
            .sender
            .send(Message::Notification(Notification {
                method: <lsp_types::notification::PublishDiagnostics as lsp_types::notification::Notification>::METHOD.to_owned(),
                params: serde_json::to_value(params).unwrap(),
            }))
            .unwrap();
    }

    fn handle_open(
        &mut self,
        params: DidOpenTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        self.commit = GitCommitDocument::new(params.text_document.text);
        self.publish_diagnostics(params.text_document.uri, self.commit.get_diagnostics());
        Ok(ServerLoopAction::Continue)
    }
    fn handle_close(
        &mut self,
        params: lsp_types::DidCloseTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        // clear the diagnostics for the document
        self.publish_diagnostics(params.text_document.uri, vec![]);
        Ok(ServerLoopAction::Break)
    }
    fn handle_did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        // let uri = params.text_document.uri;
        self.commit.edit(&params.content_changes);
        self.publish_diagnostics(params.text_document.uri, self.commit.get_diagnostics());
        // self.connection.sender.
        Ok(ServerLoopAction::Continue)
    }
    fn handle_save(
        &mut self,
        params: lsp_types::DidSaveTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        // in case incremental updates are messing up the text, try to refresh on-save
        if let Some(text) = params.text {
            eprintln!("refreshing syntax tree");
            self.commit = GitCommitDocument::new(text);
            self.publish_diagnostics(params.text_document.uri, self.commit.get_diagnostics());
        }
        Ok(ServerLoopAction::Continue)
    }

    fn handle_exit(&mut self, _: ()) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        Ok(ServerLoopAction::Break)
    }
}

// request handlers for specific methods
impl Server {
    fn handle_request(
        &mut self,
        request: lsp_server::Request,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        use lsp_types::request::*;

        macro_rules! handle {
            ($method:ty => $handler:ident) => {
                if let Ok(params) = get_request_params::<$method>(&request) {
                    return Server::$handler(self, &request.id, params);
                }
            };
        }
        eprintln!("request: {:?}", request);
        handle!(SemanticTokensFullRequest => handle_token_full);
        // handle!(SemanticTokensFullDeltaRequest => handle_token_full_delta);
        // handle!(SemanticTokensRangeRequest => handle_token_range);
        // handle!(SemanticTokensRefresh => handle_token_refresh);

        handle!(Completion => handle_completion);
        // handle!(DocumentLinkRequest => handle_doc_link_request);
        // sent from the client to the server to compute commands for a given text document and range.
        // The request is triggered when the user moves the cursor into a problem marker
        // TODO: figure out how to resolve commit, issue/PR, and mention links
        // on GitHub, BitBucket, GitLab, etc.
        // handle!(CodeActionRequest => handle_code_action);
        // sent from the client to the server to compute completion items at a given cursor position
        // handle!(HoverRequest => handle_hover);
        // handle!(RangeFormatting => handle_range_formatting);
        // handle!(ResolveCompletionItem => handle_resolving_completion_item);
        // handle!(SelectionRangeRequest => handle_selection_range_request);
        // handle!(OnTypeFormatting => handle_on_type_formatting);
        // handle!(WillSaveWaitUntil => handle_will_save_wait_until);

        let response = Response {
            id: request.id,
            result: None,
            error: Some(lsp_server::ResponseError {
                code: lsp_server::ErrorCode::MethodNotFound as i32,
                message: "method not found".to_owned(),
                data: None,
            }),
        };
        eprintln!("response: {:?}", response);
        Ok(response)
    }
    fn handle_code_action(
        // TODO: implement
        &self,
        id: &RequestId,
        params: CodeActionParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        //
        todo!("code_action")
    }
    fn handle_completion(
        &self,
        id: &RequestId,
        params: CompletionParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        let position = &params.text_document_position.position;
        eprintln!(
            "completion position: line {}, column {}",
            &position.line, &position.character
        );
        eprintln!(
            "line_text:\n\t{}",
            self.commit.code.line(position.line as usize)
        );
        eprintln!("\t{}^", " ".repeat(position.character as usize));

        let mut result = vec![];
        let character_index = position.character as usize;
        if let Some(subject) = &self.commit.subject {
            if position.line as usize == subject.line_number {
                // consider completions for the cc type, scope
                subject.debug_indices();
                subject.debug_ranges();
                // Using <= since the cursor should still trigger completions if it's at the end of a range
                if character_index <= subject.type_end().char as usize {
                    // handle type completions
                    // TODO: allow configuration of types
                    result.extend(DEFAULT_TYPES.iter().map(|item| item.to_owned()));
                } else if character_index
                    <= subject.scope_end().map(|i| i.char).unwrap_or(0) as usize
                {
                    // TODO: handle scope completions
                    eprintln!("scope completions");
                } else if character_index
                    <= subject.prefix_end().map(|i| i.char).unwrap_or(0) as usize
                {
                    // TODO: suggest either a bang or a colon
                } else {
                    // in the subject message; no completions
                }
            }
        } else {
            let line = self.commit.code.line(position.line as usize).to_string(); // panics if line is out of bounds
            if let Some(c) = line.chars().next() {
                if c == '#' {
                    // this is a commented line
                    // no completions
                } else {
                    // this is a message line
                    // completions for BREAKING CHANGE:
                    // See https://www.conventionalcommits.org/en/v1.0.0/#specification
                    if character_index >= 1 && character_index <= "BREAKING CHANGE: ".len() {
                        let prefix = &line.as_str()[0..character_index];
                        let breaking_change_match =
                            if prefix == &"BREAKING-CHANGE: "[0..character_index] {
                                Some("BREAKING-CHANGE: ")
                            } else if prefix == &"BREAKING CHANGE: "[0..character_index] {
                                Some("BREAKING CHANGE: ")
                            } else {
                                None
                            };

                        if let Some(label) = breaking_change_match {
                            result.push(lsp_types::CompletionItem {
                                label: label.to_owned(), // prefer BREAKING-CHANGE to comply with git trailers
                                kind: Some(lsp_types::CompletionItemKind::KEYWORD),
                                detail: Some("a breaking API change (correlating with MAJOR in Semantic Versioning)".to_owned()),
                                text_edit: Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
                                    range: lsp_types::Range {
                                        start: lsp_types::Position {
                                            line: position.line,
                                            character: 0,
                                        },
                                        end: lsp_types::Position {
                                            line: position.line,
                                            character: label.len().try_into().unwrap(),
                                        },
                                    },
                                    new_text: label.to_owned(),
                                })),
                                ..Default::default()
                            });
                        }
                        if character_index >= 1
                            && character_index < "Signed-off-by".len()
                            && &line.as_str()[..character_index]
                                == &"Signed-off-by"[0..character_index]
                        {
                            result.push(lsp_types::CompletionItem {
                                label: "Signed-off-by:".to_owned(),
                                kind: Some(lsp_types::CompletionItemKind::KEYWORD),
                                detail: Some(
                                    "a sign-off (correlating with Signed-off-by in git trailers)"
                                        .to_owned(),
                                ),
                                text_edit: Some(lsp_types::CompletionTextEdit::Edit(
                                    lsp_types::TextEdit {
                                        range: lsp_types::Range {
                                            start: lsp_types::Position {
                                                line: position.line,
                                                character: 0,
                                            },
                                            end: lsp_types::Position {
                                                line: position.line,
                                                character: "Signed-off-by:".len() as u32,
                                            },
                                        },
                                        new_text: "Signed-off-by: ".to_owned(),
                                    },
                                )),
                                ..Default::default()
                            });
                        }

                        eprintln!("end of message completions?");
                    }
                }
            }
        }

        let result = lsp_types::CompletionList {
            is_incomplete: false,
            items: result,
        };
        let response: Response = Response {
            id: id.clone(),
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        };
        Ok(response)
    }
    fn handle_hover(
        &self,
        id: &RequestId,
        params: HoverParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        todo!("hover")
    }
    fn handle_token_full(
        &mut self,
        id: &RequestId,
        params: lsp_types::SemanticTokensParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        let result = lsp_types::SemanticTokensResult::Tokens(lsp_types::SemanticTokens {
            result_id: None,
            data: syntax_token_scopes::handle_all_tokens(&self.commit, params)?,
        });
        let result: Response = Response {
            id: id.clone(),
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        };
        Ok(result)
    }
    fn handle_doc_link_request(
        &self,
        id: &RequestId,
        params: DocumentLinkParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        todo!("doc_link_request")
    }
    fn handle_range_formatting(
        &self,
        id: &RequestId,
        params: DocumentRangeFormattingParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        todo!("range_formatting")
    }
    fn handle_resolving_completion_item(
        &self,
        id: &RequestId,
        params: CompletionItem,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        todo!("resolving_completion_item")
    }
    fn handle_selection_range_request(
        &self,
        id: &RequestId,
        params: SelectionRangeParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        todo!("selection_range_request")
    }
    fn handle_on_type_formatting(
        &self,
        id: &RequestId,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        todo!("on_type_formatting")
    }
    fn handle_will_save_wait_until(
        &self,
        id: &RequestId,
        params: WillSaveTextDocumentParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        todo!("will_save_wait_until")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    // let tracer = logging::sdk::export::trace::stdout::new_pipeline().install_simple();
    // TODO: read in configuration about how to connect, scopes, etc.
    // lsp_server::Connection::initialize(&self, server_capabilities);
    // let conn = lsp_server::Connection::connect(addr);
    Server::from_stdio().init()?.serve()?;
    eprintln!("done");
    Ok(())
}
