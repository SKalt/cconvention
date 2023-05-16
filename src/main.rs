use crop::{self, Rope};
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

mod syntax_token_scopes;

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref CAPABILITIES: lsp_types::ServerCapabilities = get_capabilities();
    // static ref SEMANTIC_TOKEN_LEGEND: Vec<lsp_types::SemanticTokenType> = {
    //     &HIGHLIGHTS_QUERY
    // }
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

/// indices of significant characters in a conventional commit header
/// 0s indicate that the character is not present
#[derive(Debug, Default, Clone, Copy)]
struct CCIndices {
    // TODO: compact to u8? Subjects should never be more than 255 chars
    open_paren: Option<usize>,
    close_paren: Option<usize>,
    bang: Option<usize>,
    colon: Option<usize>,
    space: Option<usize>,
}
impl CCIndices {
    /// parse a conventional commit header into its byte indices
    /// ```txt
    /// type(scope)!: subject
    ///     (    )!:
    /// ```
    fn new(header: &str) -> Self {
        let mut indices = Self::default();
        let pre = if let Some(colon) = header.find(':') {
            indices.colon = Some(colon);
            &header[..colon]
        } else {
            header
        };

        indices.bang = pre.find('!');
        if let Some(open_paren) = pre.find(':') {
            indices.open_paren = Some(open_paren);
            let mid = &pre[open_paren..];
            let start = open_paren;
            indices.close_paren = mid.find(')').map(|i| i + start);

            let (mid, start) = if let Some(close_paren) = indices.close_paren {
                (&pre[close_paren..], close_paren)
            } else {
                (mid, start)
            };

            indices.space = mid.find(' ').map(|i| i + start);
        }
        indices
    }
    /// returns the byte index of the end of the type:
    /// ```txt
    /// type(scope): <subject>
    ///    ^
    /// ```
    fn type_end(&self) -> Option<usize> {
        self.open_paren.or(self.bang).or(self.colon).or(self.space)
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
    message_lines: Bitmask,
    subject_line_index: Option<usize>,
    cc_indices: CCIndices,
}
/// find the char index of the first instance of a character in a string
fn find_index(s: &str, ch: char) -> Option<usize> {
    for (i, c) in s.char_indices() {
        if c == ch {
            return Some(i);
        }
    }
    None
}

/// given a line/column position in the text, return the the byte offset of the position
fn find_byte_offset(text: &Rope, pos: Position) -> Option<usize> {
    let mut byte_offset: usize = 0;
    // only do the conversions once
    let line_index = pos.line as usize;
    let char_index = pos.character as usize;
    for (i, line) in text.raw_lines().enumerate() {
        // includes line breaks
        if i == line_index {
            // don't include the target line
            break;
        }
        byte_offset += line.byte_len();
    }
    for (i, c) in text.line(line_index).chars().enumerate() {
        byte_offset += c.len_utf8();
        // include the target char in the byte-offset
        if i == char_index {
            break;
        }
    }
    Some(byte_offset)
}

fn find_subject_line_index(message_lines: &Bitmask) -> Option<usize> {
    message_lines.iter_indices().next()
}

fn find_message_lines(code: &Rope) -> Bitmask {
    let mut message_lines = Bitmask::new(code.line_len());
    for (i, line) in code.lines().enumerate() {
        if line.bytes().next() == Some(b'#') {
        } else {
            message_lines.set(i);
        }
    }
    message_lines
}

impl SyntaxTree {
    fn new(code: String) -> Self {
        let code = crop::Rope::from(code);

        let message_lines = find_message_lines(&code);
        let subject_line_index = find_subject_line_index(&message_lines);
        let cc_indices = if let Some(subject_line_index) = subject_line_index {
            CCIndices::new(code.line(subject_line_index).to_string().as_str())
        } else {
            CCIndices::default()
        };
        SyntaxTree {
            code,
            message_lines,
            subject_line_index,
            cc_indices,
        }
    }
    fn recompute_indices(&mut self) {
        self.cc_indices = if let Some(subject_line_index) = self.subject_line_index {
            CCIndices::new(self.code.line(subject_line_index).to_string().as_str())
        } else {
            CCIndices::default()
        };
    }
    fn edit(&mut self, edits: &[TextDocumentContentChangeEvent]) -> &mut Self {
        for edit in edits {
            let range = edit.range.unwrap();
            let offset = find_byte_offset(&self.code, range.start);
            debug_assert!(
                offset.is_some(),
                "failed to find offset for {:?}",
                range.start
            );
            let start_byte = offset.unwrap();
            let end_byte = find_byte_offset(&self.code, range.end).unwrap();
            self.code.replace(start_byte..end_byte, &edit.text);

            // update the semantic ranges --------------------------------------
            // do a complete refresh of the bitmasks for simplicity
            self.update_line_bitmasks();
            self.subject_line_index = find_subject_line_index(&self.message_lines);
            if let Some(subject_line_index) = self.subject_line_index {
                if range.start.line as usize <= subject_line_index
                    && range.end.line as usize >= subject_line_index
                {
                    // also completely refresh the cc indices
                    self.recompute_indices();
                }
            }
        }

        self
    }

    fn update_line_bitmasks(&mut self) {
        let message_lines = find_message_lines(&self.code);
        self.message_lines = message_lines;
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
        let code = self.syntax_tree.code.line(position.line as usize);
        eprintln!("line_text:\n\t{}", code);
        eprintln!("\t{}^", " ".repeat(position.character as usize));
        // let mut result: lsp_types::CompletionList = lsp_types::CompletionList {
        //     is_incomplete: false,
        //     items: vec![],
        // };
        if let Some(subject_line) = self.syntax_tree.subject_line_index {}
        if self.syntax_tree.subject_line_index.is_none() {
            // no subject line -- no completions
        } else if position.line as usize == self.syntax_tree.subject_line_index.unwrap_or(0) {
            // consider completions for the cc type, scope
            let character = position.character as usize;
            // FIXME: convert byte -> char indices
            if self.syntax_tree.cc_indices.colon.is_none()
                || self.syntax_tree.cc_indices.colon.unwrap() > character
            {
                if let Some(open_paren) = self.syntax_tree.cc_indices.open_paren {
                    if open_paren > character {
                        // consider completions for the cc type
                    } else if open_paren < character {
                        // consider completions for the scope
                    } else {
                        // at the open paren; no completions
                    }
                }
            }
        } else {
            if self.syntax_tree.message_lines.get(position.line as usize) {
                // the line is commented -- no completions
            } else {
                // this is a message line
                // TODO: DCO, other trailer completions?
            }
        }

        if position.line == 0 {
            // consider completions for the cc type, scope
            // if position.character as usize < self.syntax_tree.cc_indices.colon
            //     || self.syntax_tree.cc_indices.colon == 0
            // {
            //     if self.syntax_tree.cc_indices.open_paren >= self.syntax_tree.cc_indices.close_paren
            //     {
            //         // either we're missing a scope or there's a typo. Either way,
            //     } else if position.character < self.syntax_tree.cc_indices.open_paren {
            //         // consider completions for the cc type
            //         // TODO: handle typos like "typescope): ..."
            //         // TODO: handle typos
            //         let prompt = &line_text[0..&self.syntax_tree.cc_indices.open_paren];
            //     } else if position.character < self.syntax_tree.cc_indices.close_paren {
            //         // consider completions for the cc scope
            //     }
            // } else {
            // }

            // (source (ERROR))
            // (source (subject (prefix (type) (scope))) (...))
            // let response: Response = Response {
            //     id: id.clone(),
            //     result: Some(serde_json::Value::Null),
            //     error: None,
            // };
            // return Ok(response);
        }
        let result = lsp_types::CompletionList {
            is_incomplete: false,
            items: vec![],
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
