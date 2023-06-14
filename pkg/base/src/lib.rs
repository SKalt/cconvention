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

pub mod config;
pub mod document;
pub mod server;
pub mod syntax_token_scopes;
extern crate serde_json;

lazy_static! {
    static ref LANGUAGE: tree_sitter::Language = tree_sitter_gitcommit::language();
}
