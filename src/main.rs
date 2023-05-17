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
use std::num;

mod syntax_token_scopes;

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref CAPABILITIES: lsp_types::ServerCapabilities = get_capabilities();
    static ref LANGUAGE: tree_sitter::Language = tree_sitter_gitcommit::language();
    static ref SUBJECT_QUERY: tree_sitter::Query =
        tree_sitter::Query::new(LANGUAGE.clone(), "(subject) @subject",).unwrap();
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
        selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
        completion_provider: Some(lsp_types::CompletionOptions {
            resolve_provider: None,
            trigger_characters: None,
            all_commit_characters: None,
            work_done_progress_options: lsp_types::WorkDoneProgressOptions {
                work_done_progress: None,
            },
            completion_item: None,
        }),
        signature_help_provider: None,
        definition_provider: None,
        type_definition_provider: None,
        implementation_provider: None, // maybe later, for jumping into config
        references_provider: None,
        // document_highlight_provider: Some(lsp_types::OneOf::Left(true)), // <- TODO: figure out what this does
        document_highlight_provider: None,
        document_symbol_provider: None,
        workspace_symbol_provider: None,
        code_action_provider: None,
        // FIXME: provide code actions
        // code_action_provider: Some(lsp_types::CodeActionProviderCapability::Options(
        //     lsp_types::CodeActionOptions {
        //         code_action_kinds: Some(vec![
        //             CodeActionKind::REFACTOR,
        //             CodeActionKind::SOURCE_FIX_ALL,
        //         ]),
        //         work_done_progress_options: lsp_types::WorkDoneProgressOptions {
        //             work_done_progress: None,
        //         },
        //         resolve_provider: None, // TODO: ???
        //     },
        // )),
        code_lens_provider: None, // maybe later
        document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
        document_range_formatting_provider: None,
        document_on_type_formatting_provider: Some(lsp_types::DocumentOnTypeFormattingOptions {
            first_trigger_character: "\n".to_string(),
            more_trigger_character: None,
        }),
        rename_provider: None,
        document_link_provider: None,
        // FIXME: parse URIs via tree-sitter
        // document_link_provider: Some(lsp_types::DocumentLinkOptions {
        //     resolve_provider: Some(true),
        //     work_done_progress_options: lsp_types::WorkDoneProgressOptions {
        //         work_done_progress: None,
        //     },
        // }),
        color_provider: None,           // we'll never show a color picker
        folding_range_provider: None,   // TODO: actually do this though
        declaration_provider: None,     // maybe later, for jumping to configuration
        execute_command_provider: None, // maybe later for executing code blocks
        workspace: None,                // maybe later, for git history inspection
        call_hierarchy_provider: None,
        semantic_tokens_provider: Some(
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
        ), // <- provides syntax highlighting!
        moniker_provider: None, // no real need to search for symbols in language indices
        inline_value_provider: None,
        inlay_hint_provider: None, // prefer dropdown, hover for providing info
        linked_editing_range_provider: None,
        experimental: None,
    }
}

/// char indices of significant characters in a conventional commit header.
/// Used to parse the header into its constituent parts and to find relevant completions.
#[derive(Debug, Default, Clone, Copy)]
struct CCIndices {
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
    fn new(header: &str) -> Self {
        let mut indices = Self::default();
        // type(scope)!: subject
        // type(scope)! subject
        let prefix = if let Some(colon) = header.find(':') {
            indices.colon = header.chars().position(|c| c == ':');
            &header[..colon]
        } else {
            header
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
    /// returns the char index of the end of the type:
    /// ```txt
    /// type(scope): <subject>
    ///    ^
    /// ```
    fn type_end(&self) -> Option<usize> {
        self.open_paren.or(self.bang).or(self.colon).or(self.space)
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
    /// returns the byte range of the scope in the cc subject
    /// ```txt
    /// type(scope): <subject>
    ///      ^^^^^
    /// ```
    fn scope(&self) -> Option<std::ops::Range<usize>> {
        let start = self.open_paren?;
        let end = self
            .close_paren
            .or(self.bang)
            .or(self.colon)
            .or(self.space)?;
        Some(start as usize..end as usize)
    }
}

struct Bitmask(Vec<u8>);
impl Bitmask {
    fn new(size: usize) -> Self {
        Self(Vec::with_capacity(size))
    }
    fn set(&mut self, index: usize) {
        let byte_index = index / 8;
        let bit_index = index % 8;
        if self.0.len() <= byte_index {
            self.0.resize(byte_index + 1, 0);
        }
        self.0[byte_index] |= 1 << bit_index;
    }
    fn get(&self, index: usize) -> bool {
        let byte_index = index / 8;
        let bit_index = index % 8;
        if self.0.len() <= byte_index {
            return false;
        }
        self.0[byte_index] & (1 << bit_index) != 0
    }

    // TODO: handle resizing

    fn iter_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(byte_index, byte)| {
            (0..8)
                .filter(move |bit_index| byte & (1 << bit_index) != 0)
                .map(move |bit_index| byte_index * 8 + bit_index)
        })
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
        let cc_indices = if let Some((subject, _)) = get_subject_line(&code) {
            CCIndices::new(subject.to_string().as_str())
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
        self.cc_indices = if let Some((subject, _)) = get_subject_line(&self.code) {
            CCIndices::new(subject.to_string().as_str())
        } else {
            CCIndices::default()
        };
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
        // DidCloseTextDocument
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
            if character_index < self.syntax_tree.cc_indices.type_end().unwrap_or(0) {
                // handle type completions
                eprintln!("type completions");
            } else if character_index < self.syntax_tree.cc_indices.scope_end().unwrap_or(0) {
                // handle scope completions
                eprintln!("scope completions");
            } else if character_index < self.syntax_tree.cc_indices.prefix_end().unwrap_or(0) {
                // suggest either a bang or a colon
                eprintln!("end of prefix completions?");
            } else {
                // in the message; no completions
                eprintln!("message, no completions");
            }
        } else {
            let line = self.syntax_tree.code.line(position.line as usize); // panics if line is out of bounds
            if let Some(c) = line.chars().next() {
                if c == '#' {
                    // this is a commented line
                    // no completions
                } else {
                    // this is a message line
                    // completions for BREAKING CHANGE:
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
