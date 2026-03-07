#!/usr/bin/env bash
# gj shell integration for bash
# Usage: eval "$(git-jump init bash)"

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

  local result
  result="$(__git_jump_bin "${debug_args[@]}" jump "${args[@]}")" || return $?
  if [[ -n "$result" ]]; then
    local prev_logo="${_GIT_JUMP_LOGO_TEXT:-}"
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

  local target
  target="$(__git_jump_bin "${debug_args[@]}" clone "${args[@]}")" || return $?
  builtin cd -- "$target" || return $?
  local rc
  gj "${debug_args[@]}" .
  rc=$?
  if [[ $rc -ne 0 ]]; then
    echo "gjclone: repository cloned successfully, but environment setup failed" >&2
    return $rc
  fi
}

_gj_completions() {
  local cur="${COMP_WORDS[COMP_CWORD]}"
  local candidates
  candidates="$(__git_jump_bin completions bash "$cur" 2>/dev/null)"
  if [[ -n "$candidates" ]]; then
    mapfile -t COMPREPLY <<<"$candidates"
  fi
}
complete -F _gj_completions gj
