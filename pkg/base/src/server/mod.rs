use crate::{config, syntax_token_scopes};
use crate::{config::Config, document::GitCommitDocument};
use core::panic;
use lsp_server::{self, Message, Notification, RequestId, Response};
use lsp_types::{
    self, CompletionParams, DidOpenTextDocumentParams, DocumentLinkParams,
    DocumentOnTypeFormattingParams, HoverParams, InitializeResult, ServerInfo, Url,
};
use lsp_types::{DidChangeTextDocumentParams, ServerCapabilities};
use std::collections::HashMap;
use std::error::Error;

lazy_static! {
    pub static ref CAPABILITIES: lsp_types::ServerCapabilities = {
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
            hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
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
            // TODO: provide code actions?
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
                first_trigger_character: "(".to_string(),
                more_trigger_character: None,
            }),

            document_link_provider: Some(lsp_types::DocumentLinkOptions {
                resolve_provider: Some(true),
                work_done_progress_options: lsp_types::WorkDoneProgressOptions {
                    work_done_progress: None,
                },
            }),
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
                        legend: lsp_types::SemanticTokensLegend {
                            token_types: syntax_token_scopes::SYNTAX_TOKEN_LEGEND
                                .iter()
                                .map(|tag| lsp_types::SemanticTokenType::new(tag))
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
            // useless implementation commented :/
            // selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
            ..Default::default()
        }
    };
}
/// a Server instance owns a `lsp_server::Connection` instance and a mutable
/// syntax tree, representing an actively edited .git/GIT_COMMIT_EDITMSG file.
pub struct Server<Cfg: Config> {
    config: Cfg,
    commits: HashMap<lsp_types::Url, GitCommitDocument>,
    connection: lsp_server::Connection,
}

pub enum ServerLoopAction {
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
impl<Cfg: Config> Server<Cfg> {
    /// communicate the server's capabilities with the client
    pub fn init(
        &mut self,
        cap: &ServerCapabilities,
    ) -> Result<&mut Self, Box<dyn Error + Send + Sync>> {
        span!(tracing::Level::INFO, "init");
        // let capabilities = &params.capabilities;
        let (id, _) = self.connection.initialize_start()?;
        let response = InitializeResult {
            capabilities: cap.clone(),
            server_info: Some(ServerInfo {
                name: "conventional-commit-language-server".to_owned(),
                // https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
                version: std::option_env!("CARGO_PKG_VERSION").map(|s| s.to_string()),
            }),
        };
        self.connection
            .initialize_finish(id, serde_json::json!(response))?;
        Ok(self)
    }

    /// create a fresh server with a stdio-based connection.
    pub fn from_stdio(config: Cfg) -> Self {
        let (conn, _io) = lsp_server::Connection::stdio();
        Server {
            config,
            commits: HashMap::with_capacity(1), // expect that most of the time there will be exactly 1 document
            connection: conn,
        }
    }
    pub fn serve(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        log_info!("starting server loop");
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
        span!(tracing::Level::INFO, "handle_message");
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
            Message::Response(_resp) => Ok(ServerLoopAction::Continue),
            Message::Notification(notification) => self.handle_notification(notification),
        }
    }
}

// notification handlers
impl<Cfg: Config> Server<Cfg> {
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
        span!(tracing::Level::INFO, "publish_diagnostics");
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
        let uri = params.text_document.uri;
        self.commits.insert(
            uri.clone(),
            GitCommitDocument::new(params.text_document.text),
        );
        let commit = self.commits.get(&uri).unwrap();
        self.publish_diagnostics(uri, self.config.lint(commit));
        Ok(ServerLoopAction::Continue)
    }
    fn handle_close(
        &mut self,
        params: lsp_types::DidCloseTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        // clear the diagnostics for the document
        let uri = params.text_document.uri;
        self.commits.remove(&uri);
        self.publish_diagnostics(uri, vec![]);
        // TODO: shut down the server if 0 documents are open. Unfortunately,
        // the client has to tell the server to exit.
        if self.commits.is_empty() {
            Ok(ServerLoopAction::Break)
        } else {
            Ok(ServerLoopAction::Continue)
        }
    }
    fn handle_did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        let uri = params.text_document.uri;
        let diagnostics = {
            let commit = self.commits.get_mut(&uri);
            if commit.is_none() {
                // return Err(Box::new(Error::format!("No document {uri}")));
                panic!("No document {uri}")
            }
            let commit = commit.unwrap();
            commit.edit(&params.content_changes);
            self.config.lint(commit)
        };
        self.publish_diagnostics(uri, diagnostics);
        return Ok(ServerLoopAction::Continue);
    }
    fn handle_save(
        &mut self,
        params: lsp_types::DidSaveTextDocumentParams,
    ) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        // in case incremental updates are messing up the text, try to refresh on-save
        if let Some(text) = params.text {
            let uri = params.text_document.uri;
            log_debug!("refreshing syntax tree");
            let commit = GitCommitDocument::new(text);
            self.publish_diagnostics(uri.clone(), self.config.lint(&commit));
            self.commits.insert(uri, commit);
        }
        Ok(ServerLoopAction::Continue)
    }

    fn handle_exit(&mut self, _: ()) -> Result<ServerLoopAction, Box<dyn Error + Send + Sync>> {
        Ok(ServerLoopAction::Break)
    }
}

// request handlers for specific methods
impl<Cfg: Config> Server<Cfg> {
    fn handle_request(
        &mut self,
        request: lsp_server::Request,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        span!(tracing::Level::INFO, "handle_request");
        use lsp_types::request::*;

        macro_rules! handle {
            ($method:ty => $handler:ident) => {
                if let Ok(params) = get_request_params::<$method>(&request) {
                    return Server::$handler(self, &request.id, params);
                }
            };
        }
        handle!(SemanticTokensFullRequest => handle_token_full);
        // handle!(SemanticTokensFullDeltaRequest => handle_token_full_delta);
        // handle!(SemanticTokensRangeRequest => handle_token_range);
        // handle!(SemanticTokensRefresh => handle_token_refresh);

        handle!(Completion => handle_completion);
        handle!(Formatting => handle_formatting);
        handle!(DocumentLinkRequest => handle_doc_link_request);
        // sent from the client to the server to compute commands for a given text document and range.
        // The request is triggered when the user moves the cursor into a problem marker
        // TODO: figure out how to resolve commit, issue/PR, and mention links
        // on GitHub, BitBucket, GitLab, etc.
        // handle!(CodeActionRequest => handle_code_action);
        // sent from the client to the server to compute completion items at a given cursor position
        handle!(HoverRequest => handle_hover);
        // handle!(RangeFormatting => handle_range_formatting);
        // handle!(ResolveCompletionItem => handle_resolving_completion_item);
        // handle!(SelectionRangeRequest => handle_selection_range_request);
        handle!(OnTypeFormatting => handle_on_type_formatting);

        let response = Response {
            id: request.id,
            result: None,
            error: Some(lsp_server::ResponseError {
                code: lsp_server::ErrorCode::MethodNotFound as i32,
                message: "method not found".to_owned(),
                data: None,
            }),
        };
        // eprintln!("response: {:?}", response);
        Ok(response)
    }
    fn handle_formatting(
        &self,
        id: &RequestId,
        params: lsp_types::DocumentFormattingParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        span!(tracing::Level::INFO, "handle_formatting");
        let uri = params.text_document.uri;
        if let Some(commit) = self.commits.get(&uri) {
            let response = Response {
                id: id.clone(),
                result: Some(serde_json::to_value(commit.format()).unwrap()),
                error: None,
            };
            Ok(response)
        } else {
            panic!("No such document")
        }
    }

    fn handle_completion(
        &self,
        id: &RequestId,
        params: CompletionParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        span!(tracing::Level::INFO, "handle_completion");
        let uri = params.text_document_position.text_document.uri;
        if self.commits.get(&uri).is_none() {
            panic!("no such document {uri}")
        }
        let commit = self.commits.get(&uri).unwrap();
        let position: &lsp_types::Position = &params.text_document_position.position;
        log_debug!(
            "completion position: line {}, column {}",
            &position.line,
            &position.character
        );
        log_debug!("completion context:");
        log_debug!("\t{}v", " ".repeat(position.character as usize));
        log_debug!("\t{}", commit.code.line(position.line as usize));

        let mut result = vec![];
        let character_index = position.character as usize;
        if let Some(subject) = &commit.subject {
            if position.line == subject.line_number as u32 {
                // consider completions for the cc type, scope
                // log_debug!("\t{}", subject.debug_ranges());
                // Using <= since the cursor should still trigger completions if it's at the end of a range
                let type_len = subject.type_text().chars().count();
                let scope_len = subject.scope_text().chars().count();
                let rest_len = subject.rest_text().chars().count();
                if character_index <= type_len {
                    // handle type completions
                    result.extend(config::as_completion(&self.config.type_suggestions()));
                } else if character_index <= scope_len + type_len {
                    result.extend(config::as_completion(&self.config.scope_suggestions()));
                    // eprintln!("scope completions: {:?}", result);
                    if let Some(first) = result.first_mut() {
                        first.preselect = Some(true);
                    }
                } else if character_index <= rest_len + scope_len + type_len {
                    // TODO: suggest either a bang or a colon
                } else {
                    // in the subject message; no completions
                }
            }
        } else {
            let line = commit.code.line(position.line as usize).to_string(); // panics if line is out of bounds
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
                            && line.as_str()[..character_index]
                                == "Signed-off-by"[0..character_index]
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

                        log_debug!("end of message completions?");
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
    /// provide docs on-hover of types
    /// see https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_hover
    fn handle_hover(
        &self,
        id: &RequestId,
        params: HoverParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        span!(tracing::Level::INFO, "handle_hover");
        let uri = &params.text_document_position_params.text_document.uri;
        let commit = self.commits.get(uri);
        if commit.is_none() {
            panic!("no such document {uri}");
        }
        let commit = commit.unwrap();
        if let Some(subject) = &commit.subject {
            let _position = &params.text_document_position_params.position;
            if _position.line == subject.line_number as u32 {
                let _type_text = subject.type_text();
                let _type_len = _type_text.chars().count();
                if _position.character <= _type_len as u32 {
                    if let Some((_, doc)) = self
                        .config
                        .type_suggestions()
                        .iter()
                        .find(|(type_, _doc)| type_.as_str() == _type_text)
                    {
                        return Ok(Response {
                            id: id.clone(),
                            result: Some(
                                serde_json::to_value(lsp_types::Hover {
                                    contents: lsp_types::HoverContents::Markup(
                                        lsp_types::MarkupContent {
                                            kind: lsp_types::MarkupKind::Markdown,
                                            value: doc.to_owned(),
                                        },
                                    ),
                                    range: None,
                                })
                                .unwrap(),
                            ),
                            error: None,
                        });
                    }
                } else {
                    // TODO: provide on-hover docs for scopes.
                }
            }
        }
        Ok(Response {
            id: id.clone(),
            result: Some(serde_json::Value::Null),
            error: None,
        })
    }
    fn handle_token_full(
        &mut self,
        id: &RequestId,
        params: lsp_types::SemanticTokensParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        span!(tracing::Level::INFO, "handle_token_full");
        let uri = &params.text_document.uri;
        let commit = self.commits.get_mut(uri);
        if commit.is_none() {
            panic!("no such document {uri}")
        }
        let commit = commit.unwrap();
        let result = lsp_types::SemanticTokensResult::Tokens(lsp_types::SemanticTokens {
            result_id: None,
            data: syntax_token_scopes::handle_all_tokens(commit, params)?,
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
        span!(tracing::Level::INFO, "handle_doc_link_request");
        let uri = &params.text_document.uri;
        let commit = self.commits.get(uri);
        if commit.is_none() {
            panic!("no such document {uri}");
        }
        let commit = commit.unwrap();
        Ok(lsp_server::Response {
            id: id.clone(),
            result: Some(
                serde_json::to_value(commit.get_links(self.config.repo_root().unwrap())).unwrap(),
            ),
            error: None,
        })
    }
    // fn handle_range_formatting(
    //     &self,
    //     id: &RequestId,
    //     params: DocumentRangeFormattingParams,
    // ) -> Result<Response, Box<dyn Error + Send + Sync>> {
    //     todo!("range_formatting")
    // }
    // fn handle_resolving_completion_item(
    //     &self,
    //     id: &RequestId,
    //     params: CompletionItem,
    // ) -> Result<Response, Box<dyn Error + Send + Sync>> {
    //     todo!("resolving_completion_item")
    // }
    /// see https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_selectionRange
    // fn handle_selection_range_request(
    //     &self,
    //     id: &RequestId,
    //     params: SelectionRangeParams,
    // ) -> Result<Response, Box<dyn Error + Send + Sync>> {
    //     let result: Vec<lsp_types::SelectionRange> = params
    //         .positions
    //         .iter()
    //         .map(|pos| {
    //             if let Some(subject) = &self.commit.subject {
    //                 eprintln!("expanding selection range in subject: {:?}", pos);
    //                 if pos.line == subject.line_number as u32 {
    //                     let type_len = subject.type_text().chars().count();
    //                     let scope_len = subject.scope_text().chars().count();
    //                     let rest_len = subject.rest_text().chars().count();
    //                     let prefix = lsp_types::SelectionRange {
    //                         range: lsp_types::Range {
    //                             start: lsp_types::Position {
    //                                 line: pos.line,
    //                                 character: 0,
    //                             },
    //                             end: lsp_types::Position {
    //                                 line: pos.line,
    //                                 character: type_len as u32,
    //                             },
    //                         },
    //                         parent: None,
    //                     };
    //                     let _prefix = Box::new(prefix);
    //                     if pos.character <= type_len as u32 {
    //                         return lsp_types::SelectionRange {
    //                             range: lsp_types::Range {
    //                                 start: lsp_types::Position {
    //                                     line: pos.line,
    //                                     character: 0,
    //                                 },
    //                                 end: lsp_types::Position {
    //                                     line: pos.line,
    //                                     character: type_len as u32,
    //                                 },
    //                             },
    //                             parent: Some(_prefix),
    //                         };
    //                     };
    //                     if pos.character <= (type_len + scope_len) as u32 {
    //                         return lsp_types::SelectionRange {
    //                             range: lsp_types::Range {
    //                                 start: lsp_types::Position {
    //                                     line: pos.line,
    //                                     character: type_len as u32,
    //                                 },
    //                                 end: lsp_types::Position {
    //                                     line: pos.line,
    //                                     character: (type_len + scope_len) as u32,
    //                                 },
    //                             },
    //                             parent: Some(_prefix),
    //                         };
    //                     };
    //                     if pos.character <= (type_len + scope_len + rest_len) as u32 {
    //                         return lsp_types::SelectionRange {
    //                             range: lsp_types::Range {
    //                                 start: lsp_types::Position {
    //                                     line: pos.line,
    //                                     character: (type_len + scope_len) as u32,
    //                                 },
    //                                 end: lsp_types::Position {
    //                                     line: pos.line,
    //                                     character: (type_len + scope_len + rest_len) as u32,
    //                                 },
    //                             },
    //                             parent: Some(_prefix),
    //                         };
    //                     }
    //                     return lsp_types::SelectionRange {
    //                         range: lsp_types::Range {
    //                             start: lsp_types::Position {
    //                                 line: pos.line,
    //                                 character: (type_len + scope_len + rest_len) as u32,
    //                             },
    //                             end: lsp_types::Position {
    //                                 line: pos.line,
    //                                 character: subject.line.chars().count() as u32,
    //                             },
    //                         },
    //                         parent: None,
    //                     };
    //                 };
    //             };
    //             // TODO: check for belonging to a trailer
    //             Default::default()
    //         })
    //         .collect();
    //     Ok(Response {
    //         id: id.clone(),
    //         result: Some(serde_json::to_value(result).unwrap()),
    //         error: None,
    //     })
    // }
    fn handle_on_type_formatting(
        &self,
        id: &RequestId,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Response, Box<dyn Error + Send + Sync>> {
        span!(tracing::Level::INFO, "on_type_formatting");
        log_debug!("on_type_formatting: params: {:?}", params);
        let uri = &params.text_document_position.text_document.uri;
        let commit = self.commits.get(uri);
        if commit.is_none() {
            panic!("no such document {uri}");
        }
        let commit = commit.unwrap();
        let result: Vec<lsp_types::TextEdit> = commit.format();
        Ok(Response {
            id: id.clone(),
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        })
    }
}
