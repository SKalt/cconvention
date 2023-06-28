use std::path::PathBuf;

fn stringify(stdout: Vec<u8>) -> Result<String, Box<dyn std::error::Error + Sync + Send>> {
    let string = String::from_utf8(stdout)?;
    Ok(string)
}

pub fn git(args: &[&str]) -> Result<String, Box<dyn std::error::Error + Sync + Send>> {
    let output = std::process::Command::new("git")
        .arg("--no-pager")
        .args(args)
        .output()?;
    if output.status.success() {
        Ok(stringify(output.stdout)?)
    } else {
        Err(stringify(output.stderr)?.into())
    }
}

pub fn get_repo_root() -> Result<PathBuf, Box<dyn std::error::Error + Sync + Send>> {
    git(&["rev-parse", "--show-toplevel"]).map(|s| s.into())
}
