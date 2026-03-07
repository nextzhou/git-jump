#!/usr/bin/env fish
# gj shell integration for fish
# Usage: git-jump init fish | source

function gj
    set -l debug_flag
    set -l args
    for arg in $argv
        switch $arg
            case --debug
                set debug_flag --debug
            case '*'
                set -a args $arg
        end
    end

    set -l result (command git-jump $debug_flag jump $args)
    set -l exit_code $status
    if test $exit_code -ne 0
        return $exit_code
    end
    if test -n "$result"
        set -l prev_logo "$_GIT_JUMP_LOGO_TEXT"
        for line in $result
            eval $line
        end
        if test "$_GIT_JUMP_LOGO_TEXT" != "$prev_logo"
            if test -n "$_GIT_JUMP_LOGO_TEXT"
                command git-jump logo "$_GIT_JUMP_LOGO_TEXT"
            end
        end
    end
end

function gjclone
    set -l debug_flag
    set -l args
    for arg in $argv
        switch $arg
            case --debug
                set debug_flag --debug
            case '*'
                set -a args $arg
        end
    end

    set -l target (command git-jump $debug_flag clone $args)
    set -l exit_code $status
    if test $exit_code -ne 0
        return $exit_code
    end
    builtin cd -- $target
    or return $status
    gj $debug_flag .
    set -l rc $status
    if test $rc -ne 0
        echo "gjclone: repository cloned successfully, but environment setup failed" >&2
        return $rc
    end
end

complete -c gj -f -a "(command git-jump completions fish (commandline -ct) 2>/dev/null)"
