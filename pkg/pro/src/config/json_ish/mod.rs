use super::Severity;
use indexmap::IndexMap;
use serde::Deserialize;
use std::{fs, path::PathBuf};
use toml;

fn get_config_dir(
    repo_root: &PathBuf,
) -> Result<PathBuf, Box<dyn std::error::Error + Sync + Send>> {
    let config_dir = repo_root.join(".config");
    if !config_dir.exists() {
        return Err(format!("{:?} does not exist.", config_dir).into());
    }
    if !config_dir.is_dir() {
        return Err(format!("{:?} is not a directory.", config_dir).into());
    }
    Ok(config_dir)
}

fn get_file(
    config_dir: &PathBuf,
    ext: &str,
) -> Result<PathBuf, Box<dyn std::error::Error + Sync + Send>> {
    let config_file = config_dir.join(format!("commit_convention.{ext}"));
    if !config_file.exists() {
        return Err(format!("{:?} does not exist.", config_file).into());
    }
    if !config_file.is_file() {
        // ^will traverse symlinks
        return Err(format!("{:?} is not a file.", config_file).into());
    }
    Ok(config_file)
}

#[derive(Deserialize, Clone, Debug)]
pub(crate) struct Rule {
    pub severity: Severity,
    pub query: String,
    #[serde(alias = "description")]
    pub _description: String, // <- not used except to enforce documentation of rules
    pub message: String,
}
#[derive(Deserialize, Debug, Clone)]
pub(crate) struct BuiltinLengthRule {
    pub severity: Option<Severity>,
    pub max_length: Option<u16>,
}
#[derive(Deserialize, Debug, Clone)]
pub(crate) struct BuiltinRule {
    pub(crate) severity: Severity,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct JsonConfig {
    pub scopes: Option<IndexMap<String, String>>,
    pub types: Option<IndexMap<String, String>>,

    pub header_line_max_length: Option<BuiltinLengthRule>,
    pub body_line_max_length: Option<BuiltinLengthRule>,
    // pub body_max_length: Option<BuiltinLengthRule>,
    pub signed_off_by: Option<BuiltinRule>,
    pub body_leading_blank: Option<BuiltinRule>,
    pub footer_leading_blank: Option<BuiltinRule>,
    pub missing_scope: Option<BuiltinRule>,
    pub missing_body: Option<BuiltinRule>,
    pub subject_empty: Option<BuiltinRule>,
    pub missing_subject_leading_space: Option<BuiltinRule>,
    #[serde(flatten)]
    pub plugins: IndexMap<String, Rule>,
}

fn from_toml(
    config_file: PathBuf,
) -> Result<(JsonConfig, PathBuf), Box<dyn std::error::Error + Sync + Send>> {
    let config_string = fs::read_to_string(&config_file)?;
    let config = toml::from_str(&config_string)?;
    Ok((config, config_file))
}

fn from_json(
    config_file: PathBuf,
) -> Result<(JsonConfig, PathBuf), Box<dyn std::error::Error + Sync + Send>> {
    let config_string = fs::read_to_string(&config_file)?;
    let config = serde_json::from_str(&config_string)?;
    Ok((config, config_file))
}

pub(crate) fn get_config(
    repo_root: &PathBuf,
) -> Result<(JsonConfig, PathBuf), Box<dyn std::error::Error + Sync + Send>> {
    let config_dir = get_config_dir(repo_root)?;
    // TODO: hide toml ops behind #[cfg(feature = "toml_config"))]
    get_file(&config_dir, "toml").map(from_toml)
    // TODO: support yaml
    // .or_else(|| get_file(config_dir, "yaml"))
    // .or_else(|| get_file(config_dir, "yml"))
    .or_else(|_| get_file(&config_dir, "json").map(from_json))
    .or_else(|_| get_file(repo_root, "toml").map(from_toml))
    // .or_else(|| get_file(repo_root, "yaml"))
    // .or_else(|| get_file(repo_root, "yml"))
    .or_else(|_| get_file(repo_root, "json").map(from_json))?
}
