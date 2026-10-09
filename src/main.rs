mod cli;
mod shell;
mod tui;

use clap::Parser;
use cli::{Action, Cli};
use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};
use worktree_core::{Error, Manager, Result};

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("worktree: {error}");
        std::process::exit(1);
    }
}

fn destination(path: &Path, shell_file: Option<&Path>) -> Result<()> {
    if let Some(file) = shell_file {
        use std::os::unix::ffi::OsStrExt;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(file)?;
        output.write_all(path.as_os_str().as_bytes())?;
        output.write_all(&[0])?;
    } else {
        println!("{}", path.display());
    }
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    if let Some(Action::Init { shell }) = cli.command {
        print!("{}", shell::init(shell));
        return Ok(());
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| Error("HOME is not set".into()))?;
    let manager = Manager::new(home, cli.base, env::var_os("CODEX_HOME").map(PathBuf::from))?;
    let cwd = env::current_dir()?;
    let shell_file = cli.shell_file.as_deref();
    match cli.command {
        Some(Action::Create { key }) => destination(&manager.create(&cwd, &key)?.path, shell_file)?,
        Some(Action::Open { key }) => destination(&manager.resolve(&cwd, &key)?.path, shell_file)?,
        Some(Action::Fork { source, key }) => destination(
            &manager
                .fork(&manager.resolve(&cwd, &source)?.path, &key)?
                .path,
            shell_file,
        )?,
        Some(Action::Remove { key, force }) => {
            let tree = manager.resolve(&cwd, &key)?;
            manager.remove(&tree, force)?;
            eprintln!("Removed {} ({})", tree.key, tree.project);
        }
        Some(Action::List { json }) => {
            let trees = manager.discover(&cwd)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&trees).map_err(|e| Error(e.to_string()))?
                );
            } else {
                for tree in trees {
                    println!("{}\t{}\t{}", tree.project, tree.key, tree.path.display());
                }
            }
        }
        Some(Action::Register { path }) => manager.register(path.as_deref().unwrap_or(&cwd))?,
        Some(Action::Complete { kind }) => match kind.as_str() {
            "keys" => {
                let keys: BTreeSet<_> = manager
                    .discover(&cwd)?
                    .into_iter()
                    .map(|tree| tree.key)
                    .collect();
                for key in keys {
                    println!("{key}");
                }
            }
            "branches" => {
                let output = Command::new("git")
                    .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
                    .current_dir(&cwd)
                    .output()?;
                if output.status.success() {
                    io::stdout().write_all(&output.stdout)?;
                }
            }
            _ => return Err(Error("Unknown completion kind".into())),
        },
        None => {
            if let Some(path) = tui::select(&manager, &cwd)? {
                destination(&path, shell_file)?;
            }
        }
        Some(Action::Init { .. }) => unreachable!(),
    }
    Ok(())
}
