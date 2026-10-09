use crate::{Error, Result};
use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

pub(crate) fn command<I, S>(cwd: &Path, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .map_err(|e| Error(format!("Could not run Git: {e}")))
}

pub(crate) fn run<I, S>(cwd: &Path, args: I) -> Result<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = command(cwd, args)?;
    if !output.status.success() {
        return Err(Error(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(output.stdout)
}

pub(crate) fn path(bytes: &[u8]) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    Path::new(OsStr::from_bytes(bytes)).to_path_buf()
}

pub(crate) fn apply(cwd: &Path, patch: &[u8], staged: bool) -> Result<()> {
    if patch.is_empty() {
        return Ok(());
    }
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(cwd)
        .args(["apply", "--binary"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    if staged {
        command.arg("--index");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let written = child.stdin.take().unwrap().write_all(patch);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(Error(String::from_utf8_lossy(&output.stderr).trim().into()));
    }
    written?;
    Ok(())
}
