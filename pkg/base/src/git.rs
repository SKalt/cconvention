use std::path::PathBuf;

fn stringify(stdout: Vec<u8>) -> Result<String, Box<dyn std::error::Error + Sync + Send>> {
    let string = String::from_utf8(stdout)?;
    Ok(string)
}

pub fn git(
    args: &[&str],
    cwd: Option<PathBuf>,
) -> Result<String, Box<dyn std::error::Error + Sync + Send>> {
    let mut cmd = std::process::Command::new("git");
    let output = cmd
        .current_dir(cwd.unwrap_or(".".into()))
        .arg("--no-pager")
        .args(args)
        .output()?;
    if output.status.success() {
        Ok(stringify(output.stdout)?)
    } else {
        Err(stringify(output.stderr)?.into())
    }
}

pub(crate) fn to_path(
    url: &lsp_types::Url,
) -> Result<PathBuf, Box<dyn std::error::Error + Sync + Send>> {
    match url.scheme() {
        "file" => match url.to_file_path() {
            Ok(path) => Ok(path),
            Err(_) => Err(format!("bad host or file path: {url}").into()),
        },
        other => {
            log_info!("unsupported scheme: {}", other);
            Err(format!("bad scheme: {}", url.scheme()).into())
        }
    }
}

/// for paths under .git/worktrees/<name>/, returns the path to (./git/, .)
/// for paths under .git/modules/<name>/, returns (., .)
/// for paths in a submodule worktree, returns the path to root .git/modules/<name>/ dir
pub fn get_worktree_root(
    path: &PathBuf,
) -> Result<PathBuf, Box<dyn std::error::Error + Sync + Send>> {
    // let mut path = to_path(url)?;
    let mut path = path.clone();
    while !path.is_dir() {
        if !path.pop() {
            // ^no more parent directories
            return Err(format!("no parent directories for {:?}.", path).into());
        }
    }
    let canonicalize = |p: PathBuf| if p.is_relative() { path.join(p) } else { p };
    git(&["rev-parse", "--show-toplevel"], Some(path.clone()))
        .map(|p| p.into())
        .map(canonicalize)
        .or_else(
            |err| -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
                if err.to_string().contains("not a git repository") {
                    // we're not in a git repo
                    return Ok(path);
                }
                // we're not in any worktree, nor are we in a submodule's git dir
                // so are we in a worktree's git dir or a root git dir?
                let git_dir = PathBuf::from(git(&["rev-parse", "--git-dir"], Some(path)).unwrap());
                let parent = git_dir
                    .parent()
                    .ok_or(format!("no parent directories for {:?}.", git_dir))?;
                let grandparent = parent
                    .parent()
                    .ok_or(format!("no parent directories for {:?}.", parent))?;
                let grandparent_name = grandparent
                    .file_name()
                    .ok_or(format!("No file name for {:?}", grandparent))?;
                if grandparent_name
                    .to_str()
                    .ok_or(format!("Invalid unicode: {:?}", grandparent_name))?
                    == "worktrees"
                {
                    // we're in a worktree's git dir
                    let worktree_path = std::fs::read_to_string(git_dir.join("gitdir"))?;
                    return Ok(PathBuf::from(worktree_path.trim()));
                }

                git(
                    &["rev-parse", "--show-toplevel"],
                    git_dir.parent().map(|p| p.into()),
                )
                .map(|p| p.into())
            },
        )
}
