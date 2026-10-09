#!/bin/sh
set -eu

prefix=${WORKTREE_INSTALL_PREFIX:-"$HOME/.local"}
configure_shell=1
while [ "$#" -gt 0 ]; do
  case "$1" in
    --prefix) [ "$#" -ge 2 ] || { echo '--prefix needs a directory' >&2; exit 1; }; prefix=$2; shift 2 ;;
    --no-shell) configure_shell=0; shift ;;
    *) echo "Usage: sh install.sh [--prefix DIR] [--no-shell]" >&2; exit 1 ;;
  esac
done
case "$(uname -s)" in Darwin|Linux) ;; *) echo 'macOS and Linux are supported.' >&2; exit 1 ;; esac
command -v git >/dev/null || { echo 'Install Git first.' >&2; exit 1; }
if ! command -v cargo >/dev/null && [ -r "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
command -v cargo >/dev/null || { echo 'Install stable Rust from https://rustup.rs first (build time only).' >&2; exit 1; }
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$prefix"
prefix=$(CDPATH= cd -- "$prefix" && pwd)
cargo install --path "$project_dir" --locked --force --root "$prefix"
mkdir -p "$prefix/share/worktree"
quote() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
for shell_name in zsh bash; do
  init_file="$prefix/share/worktree/init.$shell_name"
  {
    printf 'export PATH=%s:"$PATH"\n' "$(quote "$prefix/bin")"
    "$prefix/bin/worktree" init "$shell_name"
  } > "$init_file"
done
if [ "$configure_shell" -eq 1 ]; then
  case "${SHELL:-/bin/sh}" in
    */zsh) rc_file="${ZDOTDIR:-$HOME}/.zshrc"; shell_name=zsh ;;
    */bash)
      shell_name=bash
      case "$(uname -s)" in Darwin) rc_file="$HOME/.bash_profile" ;; *) rc_file="$HOME/.bashrc" ;; esac ;;
    *) echo "Installed. Source $prefix/share/worktree/init.zsh or init.bash in your shell."; exit 0 ;;
  esac
  source_line="[ ! -r $(quote "$prefix/share/worktree/init.$shell_name") ] || . $(quote "$prefix/share/worktree/init.$shell_name") # worktree shell integration"
  if ! grep -Fqx -- "$source_line" "$rc_file" 2>/dev/null; then
    printf '\n%s\n' "$source_line" >> "$rc_file"
  fi
  printf 'Installed. Open a new terminal, or run:\n. %s\n' "$(quote "$prefix/share/worktree/init.$shell_name")"
else
  printf 'Installed binary: %s/bin/worktree\n' "$prefix"
fi
