# worktree

**Switch tasks in seconds. Stay in your flow.**

Create, find, and open Git worktrees without hunting for directories. Run `worktree`
for a terminal picker with fuzzy search, arrow keys, and mouse support. Built in Rust
for macOS and Linux, with support for Codex worktrees.

## Install

With Homebrew:

```sh
brew tap alex-ross/worktree https://github.com/alex-ross/worktree
brew trust --formula alex-ross/worktree/worktree
brew install --formula --HEAD alex-ross/worktree/worktree
```

Add this to `~/.zshrc` for directory switching and autocomplete:

```sh
source "$(brew --prefix worktree)/share/worktree/init.zsh"
```

For bash, use `init.bash` in `~/.bashrc` or `~/.bash_profile`.
Open a new terminal. Git is required at runtime; Rust is only needed to build.

To build from source, install [Rust](https://rustup.rs), clone the project, and run `sh install.sh`.

## Usage

Run these commands in a Git project:

```sh
worktree create foobar          # create a branch + worktree and switch to it
worktree open foobar            # switch to an existing worktree
worktree                       # search, select, and open
worktree fork foobar experiment # copy to a new branch
worktree remove experiment      # remove the worktree, keep the branch
worktree list                  # list worktrees; --json for JSON
```

Default location: `~/.worktree/<branch>/<project>`. Change it with `--base /path` or
`WORKTREE_HOME`. An existing branch is reused; otherwise, one is created from HEAD.

In the picker: **Ctrl-N** creates, **Ctrl-F** forks, and **Ctrl-D** removes.
Forking preserves staged and unstaged changes and untracked files. Removal protects
local files; `--force` discards them. Leave the worktree before removing it.

Add relative file paths or glob patterns to `.worktreeinclude` to also copy
ignored files when creating or forking, such as `.env` and `secrets/`. Symbolic
links and paths outside the project are rejected.

`worktree register /path/to/project` makes the project's worktrees visible from anywhere.
Codex worktrees are discovered under `${CODEX_HOME:-~/.codex}/worktrees`.

## Development

Backend: [`worktree-core`](crates/worktree-core), separate from the CLI and terminal picker.
Every PR is tested on macOS/Linux and Intel/ARM, including shell integration, the terminal, and installation.
Homebrew installation is tested on macOS and Linux.

```sh
cargo test --workspace --locked
docker build -f Dockerfile.test -t worktree-test-linux .
docker run --rm worktree-test-linux
```
