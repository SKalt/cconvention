// © Steven Kalt
// SPDX-License-Identifier: APACHE-2.0

#[macro_use]
extern crate lazy_static;

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::info!($($arg)*);
    };
}
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::debug!($($arg)*);
    };
}

#[macro_export]
macro_rules! span {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        let span = tracing::span!($($arg)*);
        #[cfg(feature = "tracing")]
        let _entered = span.enter();
    };
}

#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod document;
pub mod git;
pub mod server;
pub mod syntax_token_scopes; // for convenience
extern crate serde_json;

lazy_static! {
    pub static ref LANGUAGE: tree_sitter::Language = tree_sitter_gitcommit::language();
}
