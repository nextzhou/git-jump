#!/usr/bin/env zsh
# gj shell integration for zsh
# Usage: eval "$(git-jump init zsh)"

__git_jump_bin() {
  \command git-jump "$@"
}

gj() {
  local -a debug_args=()
  local -a args=()
  local arg
  for arg in "$@"; do
    case "$arg" in
      --debug) debug_args=(--debug) ;;
      *) args+=("$arg") ;;
    esac
  done

  \builtin local result
  result="$(__git_jump_bin "${debug_args[@]}" jump "${args[@]}")" || return $?
  if [[ -n "$result" ]]; then
    \builtin local prev_logo="${_GIT_JUMP_LOGO_TEXT:-}"
    eval "$result"
    if [[ "${_GIT_JUMP_LOGO_TEXT:-}" != "$prev_logo" ]]; then
      if [[ -n "${_GIT_JUMP_LOGO_TEXT:-}" ]]; then
        command git-jump logo "$_GIT_JUMP_LOGO_TEXT"
      fi
    fi
  fi
}

gjclone() {
  local -a debug_args=()
  local -a args=()
  local arg
  for arg in "$@"; do
    case "$arg" in
      --debug) debug_args=(--debug) ;;
      *) args+=("$arg") ;;
    esac
  done

  \builtin local target
  target="$(__git_jump_bin "${debug_args[@]}" clone "${args[@]}")" || return $?
  builtin cd -- "$target" || return $?
  \builtin local rc
  gj "${debug_args[@]}" .
  rc=$?
  if [[ $rc -ne 0 ]]; then
    echo "gjclone: repository cloned successfully, but environment setup failed" >&2
    return $rc
  fi
}

_gj_completions() {
  local -a candidates
  candidates=("${(@f)$(__git_jump_bin completions zsh "${words[CURRENT]}" 2>/dev/null)}")
  if (( ${#candidates} )); then
    compadd -a candidates
  fi
}
compdef _gj_completions gj
