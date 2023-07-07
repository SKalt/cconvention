use std::path::PathBuf;

use crate::{config::ConfigStore, document::GitCommitDocument, git::git};
#[cfg(feature = "tracing")]
use atty::{self, Stream};
use clap::{Arg, ArgAction, Command};
#[cfg(feature = "tracing")]
use tracing_subscriber::{self, prelude::*, util::SubscriberInitExt};

#[cfg(feature = "telemetry")]
const SENTRY_DSN: Option<&'static str> = std::option_env!("SENTRY_DSN");

// see https://doc.rust-lang.org/cargo/reference/environment-variables.html
/// the name of the bin or crate that is getting compiled
const PKG_NAME: &str = env!("CARGO_PKG_NAME");
/// the version of the pkg that is getting compiled
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
// CARGO_BIN_NAME
pub fn cli<F, Cfg: ConfigStore>(
    init: F,
    capabilities: &lsp_types::ServerCapabilities,
    #[cfg(feature = "tracing")] enable_tracing: bool,
    #[cfg(feature = "telemetry")] enable_error_reporting: bool,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>>
where
    F: Fn() -> Result<Cfg, Box<dyn std::error::Error + Sync + Send>>,
{
    // FIXME: need CLI args to toggle tracing, telemetry _separately_
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
        SENTRY_DSN.map(|dsn| {
            sentry::init((
                dsn,
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    auto_session_tracking: true,
                    traces_sample_rate: 1.0, // TODO: reduce sampling rate
                    enable_profiling: true,
                    profiles_sample_rate: 1.0, // TODO: reduce sampling rate
                    ..Default::default()
                },
            ))
        })
    } else {
        None
    };

    let cmd = Command::new(PKG_NAME).version(PKG_VERSION)
        .subcommand(Command::new("serve").arg(Arg::new("stdio").short('s').long("stdio").action(ArgAction::SetTrue).help("Communicate via stdio")).arg(Arg::new("tcp").short('t').long("tls").help("Communicate via TCP")))
        .subcommand(
            Command::new("check").infer_long_args(true)
                .arg(
                    Arg::new("file").short('f')
                        .help("A relative or absolute path to the file containing your commit message.")
                        .conflicts_with_all(["range"])
                        .value_parser(clap::value_parser!(PathBuf)),
                )
                .arg(Arg::new("range").short('r').help("A git revision range to check.")),
        );
    match cmd.get_matches().subcommand() {
        Some(("serve", sub_matches)) => {
            let cfg = init()?;
            let mut server = if sub_matches.get_flag("stdio") {
                crate::server::Server::from_stdio(cfg)
            } else if sub_matches.get_flag("tcp") {
                crate::server::Server::from_tcp(cfg, 9999)
            } else {
                unreachable!()
            };
            server.init(capabilities)?.serve()?;
            log_info!("language server terminated");
            return Ok(());
        }
        Some(("check", sub_matches)) => {
            // TODO: use a well-known format rather than whatever this is
            // see https://eslint.org/docs/latest/use/formatters/ for inspiration
            let cfg = init()?.get(None)?;
            if let Some(file) = sub_matches.get_one::<PathBuf>("file") {
                if !file.exists() {
                    return Err(format!("{} does not exist", file.display()).into());
                }
                if !file.is_file() {
                    return Err(format!("{} is not a file", file.display()).into());
                }
                let text = std::fs::read_to_string(&file)?;
                let doc = GitCommitDocument::new()
                        // .with_url(&lsp_types::Url::from_file_path(&file).map_err(|_| {
                        //     format!("unable to construct url from {}", file.display())
                        // })?)
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
                    let start_line = d.range.start.line + 1;
                    let start_column = d.range.start.character + 1;
                    println!(
                        "{}\t{:?}:{}:{}\t{}\t{}", // TODO: colorize if a tty
                        &file.display(),
                        d.severity.unwrap(),
                        start_line,
                        start_column,
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
                    diagnostics_for_hash
                        .iter()
                        .for_each(|d: &lsp_types::Diagnostic| {
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
                if error_count > 0 {
                    return Err("errors found".into());
                }
            }
            return Ok(());
        }
        Some((sub_command, _)) => {
            return Err(format!("unexpected subcommand {}", sub_command).into())
        }
        None => unreachable!(),
    };
}
