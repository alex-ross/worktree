use crate::cli::Shell;

pub fn init(shell: Shell) -> String {
    let wrapper = r#"worktree() {
  local worktree_file worktree_destination worktree_status
  worktree_file=$(mktemp "${TMPDIR:-/tmp}/worktree-shell.XXXXXXXX") || return
  if command worktree --shell-file "$worktree_file" "$@"; then
    worktree_status=0
    if IFS= read -r -d '' worktree_destination < "$worktree_file"; then
      builtin cd -- "$worktree_destination" || worktree_status=$?
    fi
  else
    worktree_status=$?
  fi
  command rm -f -- "$worktree_file"
  return "$worktree_status"
}
"#;
    let completion = match shell {
        Shell::Zsh => {
            r#"
_worktree() {
  local context state state_descr line
  typeset -A opt_args
  local -a keys branches commands
  _arguments -C \
    '(-b --base)'{-b,--base}'[Base directory]:directory:_files -/' \
    '--help[Show help]' '--version[Show version]' \
    '1:command:->command' '*::argument:->argument'
  case "$state" in
    command)
      commands=('create:Create a worktree' 'open:Open a worktree'
        'fork:Fork a worktree' 'remove:Remove a worktree'
        'list:List worktrees' 'register:Remember a project' 'init:Shell integration')
      _describe 'command' commands ;;
    argument)
      case "$line[1]" in
        open|remove|rm|fork)
          if [[ "$line[1]" == fork && CURRENT -gt 2 ]]; then
            branches=("${(@f)$(command worktree complete branches 2>/dev/null)}")
            compadd -- "${branches[@]}"
          else
            keys=("${(@f)$(command worktree complete keys 2>/dev/null)}")
            compadd -- "${keys[@]}"
            [[ "$line[1]" == remove || "$line[1]" == rm ]] && _arguments '--force[Discard local files]'
          fi ;;
        create)
          branches=("${(@f)$(command worktree complete branches 2>/dev/null)}")
          compadd -- "${branches[@]}" ;;
        register) _files -/ ;;
        init) compadd zsh bash ;;
        list) _arguments '--json[Output JSON]' ;;
      esac ;;
  esac
}
(( $+functions[compdef] )) || { autoload -Uz compinit; compinit; }
compdef _worktree worktree
"#
        }
        Shell::Bash => {
            r#"
_worktree() {
  local current="${COMP_WORDS[COMP_CWORD]}" item
  COMPREPLY=()
  if [[ "${COMP_WORDS[COMP_CWORD-1]}" == --base || "${COMP_WORDS[COMP_CWORD-1]}" == -b ]]; then
    while IFS= read -r item; do COMPREPLY+=("$item"); done < <(compgen -d -- "$current")
    return
  fi
  if (( COMP_CWORD == 1 )); then
    COMPREPLY=($(compgen -W 'create open fork remove list register init --base --help --version' -- "$current"))
    return
  fi
  case "${COMP_WORDS[1]}" in
    create) item=branches ;;
    open|remove|rm) item=keys ;;
    fork) if (( COMP_CWORD == 2 )); then item=keys; else item=branches; fi ;;
    init) COMPREPLY=($(compgen -W 'zsh bash' -- "$current")); return ;;
    list) COMPREPLY=($(compgen -W '--json' -- "$current")); return ;;
    register) while IFS= read -r item; do COMPREPLY+=("$item"); done < <(compgen -d -- "$current"); return ;;
    *) return ;;
  esac
  while IFS= read -r item; do
    [[ "$item" == "$current"* ]] && COMPREPLY+=("$item")
  done < <(command worktree complete "$item" 2>/dev/null)
  [[ "${COMP_WORDS[1]}" == remove && --force == "$current"* ]] && COMPREPLY+=(--force)
}
complete -F _worktree worktree
"#
        }
    };
    format!("{wrapper}{completion}")
}
