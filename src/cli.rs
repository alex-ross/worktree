use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about = "Git worktrees, one command away")]
pub struct Cli {
    /// Parent directory for new worktrees (default: ~/.worktree)
    #[arg(long, short = 'b', env = "WORKTREE_HOME", global = true)]
    pub base: Option<PathBuf>,
    #[arg(long, hide = true, global = true)]
    pub shell_file: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Action>,
}

#[derive(Subcommand, Debug)]
pub enum Action {
    /// Create a worktree and check out KEY, creating the branch if needed
    Create { key: String },
    /// Open an existing worktree by branch/key or full path
    Open { key: String },
    /// Fork SOURCE into a new branch and worktree
    Fork { source: String, key: String },
    /// Remove a worktree (keeps the branch)
    #[command(alias = "rm")]
    Remove {
        key: String,
        /// Discard changed, untracked and ignored files
        #[arg(long)]
        force: bool,
    },
    /// List all discovered worktrees
    List {
        #[arg(long)]
        json: bool,
    },
    /// Remember a project so its worktrees appear from any directory
    Register { path: Option<PathBuf> },
    /// Print shell integration and dynamic completions
    Init {
        #[arg(value_enum)]
        shell: Shell,
    },
    #[command(hide = true)]
    Complete { kind: String },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Shell {
    Zsh,
    Bash,
}
