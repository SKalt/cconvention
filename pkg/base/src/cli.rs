use std::path::PathBuf;

use crate::{config::ConfigStore, document::GitCommitDocument, git::git};
#[cfg(feature = "tracing")]
use atty::{self, Stream};
use clap::{Arg, Command};
#[cfg(feature = "tracing")]
use tracing_subscriber::{self, prelude::*, util::SubscriberInitExt};

#[cfg(feature = "telemetry")]
const SENTRY_DSN: &'static str = std::env!("SENTRY_DSN", "no $SENTRY_DSN set");

pub fn cli<F, Cfg: ConfigStore>(
    init: F,
    capabilities: &lsp_types::ServerCapabilities,
    #[cfg(feature = "tracing")] enable_tracing: bool,
    #[cfg(feature = "telemetry")] enable_error_reporting: bool,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>>
where
    F: Fn() -> Result<Cfg, Box<dyn std::error::Error + Sync + Send>>,
{
    #[cfg(feature = "tracing")]
    {
        let reg = tracing_subscriber::Registry::default().with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(atty::is(Stream::Stderr))
                .with_filter(tracing_subscriber::filter::filter_fn(|meta| {
                    match meta.module_path() {
                        Some(path) => path.starts_with(module_path!()),
                        None => false,
                    }
                })),
        );
        #[cfg(feature = "telemetry")]
        {
            if enable_tracing {
                reg.with(sentry::integrations::tracing::layer()).init();
            } else {
                reg.init();
            };
        }
        #[cfg(not(feature = "telemetry"))]
        reg.init();
    };
    #[cfg(feature = "telemetry")]
    let _guard = if enable_error_reporting {
        Some(sentry::init((
            SENTRY_DSN,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                auto_session_tracking: true,
                traces_sample_rate: 1.0, // TODO: reduce sampling rate
                enable_profiling: true,
                profiles_sample_rate: 1.0, // TODO: reduce sampling rate
                ..Default::default()
            },
        )))
    } else {
        None
    };

    let cmd = Command::new(env!("CARGO_PKG_NAME"))
        .subcommand(Command::new("serve"))
        .subcommand(
            Command::new("check")
                .arg(
                    Arg::new("file").short('f').long("file")
                        .help("A relative or absolute path to the file containing your commit message.")
                        .conflicts_with_all(["range"])
                        .value_parser(clap::value_parser!(PathBuf)),
                )
                .arg(Arg::new("range").short('r').long("range").help("A git revision range to check.")),
        ).subcommand_required(true);
    match cmd.get_matches().subcommand() {
        Some(("serve", _)) => {
            let cfg = init()?;
            crate::server::Server::from_stdio(cfg)
                .init(capabilities)?
                .serve()?;
            log_info!("language server terminated");
            return Ok(());
        }
        Some(("check", sub_matches)) => {
            // TODO: use a well-known format rather than whatever this is
            let cfg = init()?.get(None)?;
            if let Some(file) = sub_matches.get_one::<PathBuf>("file") {
                let file = file.canonicalize()?;
                if !file.exists() {
                    return Err(format!("{} does not exist", file.display()).into());
                }
                if !file.is_file() {
                    return Err(format!("{} is not a file", file.display()).into());
                }
                let text = std::fs::read_to_string(&file)?;
                let doc =
                    GitCommitDocument::new()
                        .with_url(&lsp_types::Url::from_file_path(&file).map_err(|_| {
                            format!("unable to construct url from {}", file.display())
                        })?)
                        .with_text(text);
                let diagnostics = cfg.lint(&doc);
                let error_count = diagnostics
                    .iter()
                    .filter(|d| d.severity.unwrap() == lsp_types::DiagnosticSeverity::ERROR)
                    .count();
                let warning_count = diagnostics
                    .iter()
                    .filter(|d| d.severity.unwrap() == lsp_types::DiagnosticSeverity::WARNING)
                    .count();
                diagnostics.iter().for_each(|d: &lsp_types::Diagnostic| {
                    let code = match d.code.as_ref().unwrap() {
                        lsp_types::NumberOrString::String(s) => s,
                        _ => panic!("expected code to be a string"),
                    };
                    println!(
                        "{}\t{:?}\t{}\t{}", // TODO: colorize if a tty
                        &file.display(),
                        d.severity.unwrap(),
                        &code,
                        d.message,
                    );
                });
                println!("{} errors, {} warnings", error_count, warning_count)
            } else if let Some(range) = sub_matches.get_one::<String>("range") {
                let raw_hashes = git(&["log", "--format=%h", range], None)?;
                let hashes = raw_hashes
                    .lines()
                    .map(|line| line.trim())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                let mut diagnostics = vec![];
                // process each hash's commit message
                for hash in hashes {
                    let message = git(&["log", "-n", "1", "--format=%B", hash], None)?;
                    let doc = GitCommitDocument::new().with_text(message);
                    let diagnostics_for_hash = cfg.lint(&doc);
                    diagnostics.iter().for_each(|d: &lsp_types::Diagnostic| {
                        let code = match d.code.as_ref().unwrap() {
                            lsp_types::NumberOrString::String(s) => s,
                            _ => panic!("expected code to be a string"),
                        };
                        println!(
                            "{}\t{:?}\t{}\t{}", // TODO: colorize if a tty
                            hash,
                            d.severity.unwrap(),
                            &code,
                            d.message,
                        );
                    });
                    diagnostics.extend(diagnostics_for_hash);
                }
                let error_count = diagnostics
                    .iter()
                    .filter(|d| d.severity.unwrap() == lsp_types::DiagnosticSeverity::ERROR)
                    .count();
                let warning_count = diagnostics
                    .iter()
                    .filter(|d| d.severity.unwrap() == lsp_types::DiagnosticSeverity::WARNING)
                    .count();
                println!("{} errors, {} warnings", error_count, warning_count);
            }
            return Ok(());
        }
        Some((sub_command, _)) => {
            return Err(format!("unexpected subcommand {}", sub_command).into())
        }
        None => unreachable!(),
    };
}
