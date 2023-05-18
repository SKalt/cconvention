use crop::{self, Rope, RopeSlice};
use lsp_server::{self, Message, RequestId, Response};
use lsp_types::request::{self, RangeFormatting, SemanticTokensFullRequest, SemanticTokensRefresh};
use lsp_types::{
    self, CodeActionParams, CompletionItem, CompletionParams, DidOpenTextDocumentParams,
    DocumentHighlightParams, DocumentLinkParams, DocumentOnTypeFormattingParams,
    DocumentRangeFormattingParams, HoverParams, InitializeParams, InitializeResult, Position,
    SelectionRangeParams, SemanticTokenModifier, SemanticTokensLegend, ServerInfo,
    TextDocumentContentChangeEvent, WillSaveTextDocumentParams,
};
use lsp_types::{CodeActionKind, DidChangeTextDocumentParams};
use std::error::Error;
use std::f32::consts::E;
use std::num;

mod syntax_token_scopes;

extern crate serde_json;

#[macro_use]
extern crate lazy_static;

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
                save: None,
            },
        )),
        hover_provider: None,
        // FIXME: provide hover
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
        code_action_provider: Some(lsp_types::CodeActionProviderCapability::Options(
            lsp_types::CodeActionOptions {
                code_action_kinds: Some(vec![
                    CodeActionKind::EMPTY,
                    CodeActionKind::QUICKFIX,
                    CodeActionKind::REFACTOR,
                    CodeActionKind::SOURCE_FIX_ALL,
                ]),
                work_done_progress_options: lsp_types::WorkDoneProgressOptions {
                    work_done_progress: None,
                },
                resolve_provider: None,
            },
        )),
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

/// char indices of significant characters in a conventional commit header.
/// Used to parse the header into its constituent parts and to find relevant completions.
#[derive(Debug, Default, Clone)]
struct CCIndices {
    line: String,
    line_number: Option<usize>,
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
    open_paren: Option<usize>,
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
    close_paren: Option<usize>,
    /// the first exclamation mark in the subject line *before* the colon.
    /// ```txt
    /// ### valid ###
    /// type(scope)!: subject
    /// #          ! -> <Some(10)>
    /// type!: subject
    /// #   ! -> <Some(3)>
    /// type: subject!()
    /// # -> <None>
    bang: Option<usize>,
    colon: Option<usize>,
    space: Option<usize>,
}
impl CCIndices {
    /// parse a conventional commit header into its byte indices
    /// ```txt
    /// type(scope)!: subject
    ///     (     )!:_
    /// ```
    fn new(header: String, line_number: usize) -> Self {
        let mut indices = Self::default();
        indices.line_number = Some(line_number);
        indices.line = header;
        let line = indices.line.as_str();
        // type(scope)!: subject
        // type(scope)! subject
        let prefix = if let Some(colon) = line.find(':') {
            indices.colon = line.chars().position(|c| c == ':');
            &line[..colon]
        } else {
            line
        };

        // type(scope)!
        // type(scope) subject
        let prefix = if let Some(bang) = prefix.find('!') {
            indices.bang = prefix.chars().position(|c| c == '!');
            &prefix[..bang]
        } else {
            prefix
        };

        // type(scope
        // type(scope subject
        let prefix = if let Some(close_paren) = prefix.find(')') {
            indices.close_paren = prefix.chars().position(|c| c == ')');
            &prefix[..close_paren]
        } else {
            prefix
        };

        // type(
        // type scope subject
        // type subject
        let prefix = if let Some(open_paren) = prefix.find('(') {
            indices.open_paren = prefix.chars().position(|c| c == '(');
            &prefix[..open_paren]
        } else {
            prefix
        };

        // type scope subject
        // type subject
        indices.space = prefix.chars().position(|c| c == ' ');
        indices
    }
    /// returns the char index of the end of the type, NON-INCLUSIVE:
    /// ```txt
    /// type(scope): <subject>
    ///    ^
    /// ```
    fn type_end(&self) -> usize {
        self.open_paren
            .or(self.bang)
            .or(self.colon)
            .or(self.space)
            .unwrap_or_else(|| self.line.chars().count())
    }

    /// returns the char index of the end of the scope:
    /// ```txt
    /// type(scope): <subject>
    ///           ^
    /// type(scope: <subject>
    /// #        ^
    /// type: <subject>
    /// # -> <None>
    /// ```
    fn scope_end(&self) -> Option<usize> {
        if self.close_paren.is_none() && self.open_paren.is_none() {
            return None;
        } else {
            self.close_paren.or(self.bang).or(self.colon).or(self.space)
        }
    }

    fn prefix_end(&self) -> Option<usize> {
        self.colon.or(self.bang).or(self.close_paren).or(self.space)
    }

    fn debug_indices(&self) {
        // TODO: ensure this function call is a no-op in release builds
        eprintln!("debugging indices");
        if self.line_number.is_none() {
            eprintln!("no subject line");
            return;
        }
        let mut cursor = 0;

        eprint!("\t{}\n\t", self.line);
        if let Some(open_paren) = self.open_paren {
            while cursor < open_paren {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!("(");
        }
        if let Some(close_paren) = self.close_paren {
            while cursor < close_paren {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!(")");
        }
        if let Some(bang) = self.bang {
            while cursor < bang {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!("!"); // HACK: relying on byte indices
        }
        if let Some(colon) = self.colon {
            while cursor < colon {
                cursor += 1;
                eprint!(" ");
            }
            cursor += 1;
            eprint!(":"); // HACK: relying on byte indices
        }
        if let Some(space) = self.space {
            while cursor < space {
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
        if self.line_number.is_none() {
            eprintln!("no subject line");
            return;
        }
        eprint!("\t{}\n\t", self.line);
        let mut cursor = 0usize;
        while cursor < self.type_end() {
            cursor += 1;
            eprint!("t")
        }
        if let Some(open_paren) = self.open_paren {
            while cursor < open_paren {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!("(");
        }
        while cursor < self.scope_end().unwrap_or(0) {
            cursor += 1;
            eprint!("s");
        }
        if let Some(close_paren) = self.close_paren {
            while cursor < close_paren {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!(")");
        }
        if let Some(bang) = self.bang {
            while cursor < bang {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!("!")
        }
        if let Some(colon) = self.colon {
            while cursor < colon {
                cursor += 1;
                eprint!(" ")
            }
            cursor += 1;
            eprint!(":")
        }
        if let Some(space) = self.space {
            while cursor < space {
                cursor += 1;
                eprint!(" ")
            }
            eprint!("_")
        }
        eprintln!("\n");
    }
}

struct SyntaxTree {
    code: crop::Rope,
    parser: tree_sitter::Parser,
    tree: tree_sitter::Tree,
    cc_indices: CCIndices,
}

/// given a line/column position in the text, return the the byte offset of the position
fn find_byte_offset(text: &Rope, pos: Position) -> Option<usize> {
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
            for (i, c) in line.chars().enumerate() {
                if i == char_index {
                    // don't include the target char in the byte-offset
                    break;
                }
                byte_offset += c.len_utf8();
            }
        }
    }
    Some(byte_offset)
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

impl SyntaxTree {
    fn new(text: String) -> Self {
        let code = crop::Rope::from(text.clone());
        let cc_indices = if let Some((subject, line_number)) = get_subject_line(&code) {
            CCIndices::new(subject.to_string(), line_number)
        } else {
            CCIndices::default()
        };
        let mut parser = {
            let language = tree_sitter_gitcommit::language();
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(language).unwrap();
            parser
        };
        let tree = parser.parse(&text, None).unwrap();
        SyntaxTree {
            code,
            parser,
            tree,
            cc_indices,
        }
    }
    fn recompute_indices(&mut self) {
        self.cc_indices = if let Some((subject, line_number)) = self._get_subject_line_with_number()
        {
            CCIndices::new(subject.to_string(), line_number)
        } else {
            CCIndices::default()
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
        let matches = cursor.matches(&SUBJECT_QUERY, self.tree.root_node(), code.as_bytes());
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
        for edit in edits {
            debug_assert!(edit.range.is_some(), "range is none");
            if edit.range.is_none() {
                continue;
            }
            let range = edit.range.unwrap();
            let offset = find_byte_offset(&self.code, range.start);
            debug_assert!(
                offset.is_some(),
                "failed to find start byte-offset for {:?}",
                range.start
            );
            let start_byte = offset.unwrap();
            let end_byte = {
                let end_byte = find_byte_offset(&self.code, range.end);
                debug_assert!(
                    end_byte.is_some(),
                    "failed to find ending byte-offset for {:?}",
                    range.end
                );
                end_byte.unwrap()
            };
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
            self.tree.edit(&tree_sitter::InputEdit {
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
                let prev_tree = &self.tree;
                self.tree = self
                    .parser
                    .parse(&(self.code.to_string()), Some(prev_tree))
                    .unwrap();
                eprintln!("{}", &self.tree.root_node().to_sexp());
                // TODO: detect if the subject line changed.
                // HACK: for now, just recompute the indices
                self.recompute_indices();
            }
        }

        self
    }
}

/// a Server instance owns a `lsp_server::Connection` instance and a mutable
/// syntax tree, representing an actively edited .git/GIT_COMMIT_EDITMSG file.
struct Server {
    syntax_tree: SyntaxTree,
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
            syntax_tree: SyntaxTree::new("".to_owned()),
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
        // DidSaveTextDocument
        // DidChangeWatchedFiles
        // WorkDoneProgressCancel

        Ok(ServerLoopAction::Continue)
    }
    fn handle_open(
        &mut self,
        params: DidOpenTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        self.syntax_tree = SyntaxTree::new(params.text_document.text);
        // TODO: log debug info
        Ok(ServerLoopAction::Continue)
    }
    fn handle_close(
        &mut self,
        _: lsp_types::DidCloseTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        Ok(ServerLoopAction::Break)
    }
    fn handle_did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        // let uri = params.text_document.uri;
        self.syntax_tree.edit(&params.content_changes);
        // TODO: log debug info
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

        // handle!(DocumentHighlightRequest => handle_document_highlight);
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
        // FIXME: figure out how to return a Box<dyn Error + Send + Sync>
        // panic!("unhandled request: {:?}", request.method)
    }
    fn handle_code_action(
        // TODO: implement
        &self,
        id: &RequestId,
        params: CodeActionParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
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
            self.syntax_tree.code.line(position.line as usize)
        );
        eprintln!("\t{}^", " ".repeat(position.character as usize));

        let mut result = vec![];
        let character_index = position.character as usize;
        if position.line as usize == self.syntax_tree.get_subject_line_number() {
            // consider completions for the cc type, scope
            self.syntax_tree.cc_indices.debug_indices();
            self.syntax_tree.cc_indices.debug_ranges();
            // Using <= since the cursor should still trigger completions if it's at the end of a range
            if character_index <= self.syntax_tree.cc_indices.type_end() {
                // handle type completions
                // TODO: allow configuration of types
                result.extend(DEFAULT_TYPES.iter().map(|item| item.to_owned()));
            } else if character_index <= self.syntax_tree.cc_indices.scope_end().unwrap_or(0) {
                // TODO: handle scope completions
                eprintln!("scope completions");
            } else if character_index <= self.syntax_tree.cc_indices.prefix_end().unwrap_or(0) {
                // TODO: suggest either a bang or a colon
            } else {
                // in the subject message; no completions
            }
        } else {
            let line = self
                .syntax_tree
                .code
                .line(position.line as usize)
                .to_string(); // panics if line is out of bounds
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
                            } else if prefix == &"BREAKING CHANGE:"[0..character_index] {
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
    fn handle_document_highlight(
        //  usually highlights all references to the symbol scoped to this file.
        &self,
        id: &RequestId,
        params: DocumentHighlightParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        eprintln!("params: {:?}", params);
        // let response: lsp_types::DocumentHighlight
        let response: Response = Response {
            id: id.clone(),
            result: Some(serde_json::Value::Null),
            error: None,
        };
        todo!("document_highlight")
    }
    fn handle_token_full(
        &mut self,
        id: &RequestId,
        params: lsp_types::SemanticTokensParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        let result = lsp_types::SemanticTokensResult::Tokens(lsp_types::SemanticTokens {
            result_id: None,
            data: syntax_token_scopes::handle_all_tokens(&self.syntax_tree, params)?,
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
