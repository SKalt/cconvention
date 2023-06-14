#[macro_use]
extern crate lazy_static;

use conventional_commit_language_server_basic::{config, log_info, server, syntax_token_scopes};

#[cfg(feature = "tracing")]
use tracing_subscriber::{self, prelude::*, util::SubscriberInitExt};

use server::Server;

lazy_static! {
    static ref CAPABILITIES: lsp_types::ServerCapabilities = get_capabilities();
}

#[cfg(feature = "telemetry")]
const SENTRY_DSN: &'static str = std::env!("SENTRY_DSN", "no $SENTRY_DSN set");

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
        // useless implementation commented :/
        // selection_range_provider: Some(lsp_types::SelectionRangeProviderCapability::Simple(true)),
        ..Default::default()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    #[cfg(feature = "tracing")]
    {
        let reg = tracing_subscriber::Registry::default().with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false), // TODO: detect ttys
        );
        #[cfg(feature = "telemetry")]
        let reg = reg.with(sentry::integrations::tracing::layer());
        reg.init();
    }
    #[cfg(feature = "telemetry")]
    let _guard = sentry::init((
        SENTRY_DSN,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            auto_session_tracking: true,
            traces_sample_rate: 1.0,
            enable_profiling: true,
            profiles_sample_rate: 1.0,
            debug: true,
            ..Default::default()
        },
    ));
    log_info!("starting");
    Server::from_stdio(Box::new(config::DefaultConfig))
        .init(&CAPABILITIES)?
        .serve()?;
    log_info!("done");
    Ok(())
}
