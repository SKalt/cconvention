use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    config::{Config, ConfigStore},
    document::GitCommitDocument,
    git::git,
};
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

pub fn serve<Cfg: ConfigStore>(
    cfg: Cfg,
    sub_matches: &clap::ArgMatches,
    capabilities: &lsp_types::ServerCapabilities,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
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

pub fn check(
    cfg: Arc<dyn Config>,
    sub_matches: &clap::ArgMatches,
) -> Result<(String, u8, u8), Box<dyn std::error::Error + Sync + Send>> {
    span!(tracing::Level::INFO, "check");
    let mut result = String::new();
    let mut error_count = 0u8;
    let mut warning_count = 0u8;
    let mut write_lint = |group: &str, d: &lsp_types::Diagnostic| {
        let code = match d.code.as_ref().unwrap() {
            lsp_types::NumberOrString::String(s) => s,
            _ => panic!("expected code to be a string"),
        };
        let start_line = d.range.start.line + 1;
        let start_column = d.range.start.character + 1;
        result.push_str(&format!(
            "{}:{}:{}\t{:?}\t{}\t{}\n",
            group,
            start_line,
            start_column,
            d.severity.unwrap(),
            code,
            d.message
        ));
    };
    let diagnostics = if let Some(file) = sub_matches.get_one::<PathBuf>("file") {
        if !file.exists() {
            return Err(format!("{} does not exist", file.display()).into());
        }
        if !file.is_file() {
            return Err(format!("{} is not a file", file.display()).into());
        }
        let group = file.display().to_string();
        let text = std::fs::read_to_string(&file)?;
        let doc = GitCommitDocument::new().with_text(text);
        let diagnostics = cfg.lint(&doc);
        diagnostics.iter().for_each(|d| write_lint(&group, d));
        diagnostics
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
                .for_each(|d| write_lint(&hash, d));
            diagnostics.extend(diagnostics_for_hash);
        }
        diagnostics
    } else {
        unreachable!()
    };
    diagnostics.iter().for_each(|d| match d.severity.unwrap() {
        lsp_types::DiagnosticSeverity::ERROR => error_count += 1,
        lsp_types::DiagnosticSeverity::WARNING => warning_count += 1,
        _ => {}
    });
    Ok((result, error_count, warning_count))
}

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
                        Some(path) => path.starts_with(module_path!()) || path.starts_with("base"),
                        None => false,
                    }
                })),
        );
        #[cfg(feature = "telemetry")]
        {
            if enable_tracing {
                log_debug!("tracing enabled");
                reg.with(sentry::integrations::tracing::layer()).init();
            } else {
                log_debug!("tracing disabled");
                reg.init();
            };
        }
        #[cfg(not(feature = "telemetry"))]
        reg.init();
    };
    #[cfg(feature = "telemetry")]
    let _guard = if enable_error_reporting {
        log_debug!("error reporting enabled");
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
        log_debug!("error reporting disabled");
        None
    };

    let cmd = Command::new(PKG_NAME).version(PKG_VERSION)
        .subcommand(
            Command::new("serve").about("Run a language server")
                .arg(Arg::new("stdio").short('s').long("stdio").action(ArgAction::SetTrue).help("Communicate via stdio"))
                .arg(Arg::new("tcp").short('t').long("tls").help("Communicate via TCP")))
        .subcommand(
            Command::new("check").about("Lint commit message(s)").infer_long_args(true)
                .arg(
                    Arg::new("file").short('f')
                        .help("A relative or absolute path to the file containing your commit message.")
                        .conflicts_with_all(["range"])
                        .value_parser(clap::value_parser!(PathBuf)),
                )
                .arg(Arg::new("range").short('r').help("A git revision range to check.")),
        ).subcommand_required(true);
    match cmd.get_matches().subcommand() {
        Some(("serve", sub_matches)) => return serve(init()?, sub_matches, capabilities),
        Some(("check", sub_matches)) => {
            // TODO: use a well-known format rather than whatever this is
            // see https://eslint.org/docs/latest/use/formatters/ for inspiration
            let (message, error_count, warning_count) = check(init()?.get(None)?, sub_matches)?;
            if !message.is_empty() {
                println!("{}", message);
            }
            if error_count == 0 {
                return Ok(());
            } else {
                return Err(format!("{} errors, {} warnings", error_count, warning_count).into());
            }
        }
        Some((sub_command, _)) => {
            return Err(format!("unexpected subcommand {}", sub_command).into())
        }
        None => unreachable!(),
    };
}

// TODO: use snapshot tests of check() output
