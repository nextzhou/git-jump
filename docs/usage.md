# Usage Guide

This guide covers all `gj` commands in detail, including matching rules, TUI keybindings,
and practical examples.

---

## Jump Command

```
gj [pattern...]
```

The jump command is the core of `gj`. It finds a matching project, changes the shell's working
directory to that project, and applies all configured git config, environment variables, and
hooks.

### How Matching Works

`gj` uses **multi-token substring matching**:

1. The pattern is split into tokens by whitespace.
2. Each token must appear as a case-insensitive substring somewhere in the project's display
   path (e.g. `github.com/my-org/backend-api` or its alias equivalent).
3. All tokens must match (AND logic). A project is excluded if any token fails to match.

```bash
gj api              # matches any project containing "api"
gj api gate         # matches projects containing both "api" AND "gate"
gj org backend      # matches projects in "org" group with "backend" in the name
```

### Coverage-Based Scoring

When multiple projects match, `gj` ranks them by **coverage score**:

- **Project score**: how much of the project name is covered by the matched tokens.
  A token that covers a larger fraction of the project name scores higher.
- **Group score**: average coverage across group path components.

Projects with higher project scores appear first. This means `gj api` will rank `api` above
`api-gateway` when both match, because the token covers 100% of `api` but only part of
`api-gateway`.

### Single Match

When exactly one project matches, `gj` jumps to it immediately without showing the TUI.

```bash
gj backend-api      # jumps directly if only one project matches
```

### Multiple Matches: TUI Selector

When multiple projects match (or when called with no arguments), `gj` opens an interactive
TUI selector:

```
> api_
  github.com/my-org/backend-api
  github.com/my-org/api-gateway
  git.example.com/backend/api
```

The top bar is a live filter input. Type to narrow results in real time.

#### TUI Keybindings

| Key              | Action                                      |
|------------------|---------------------------------------------|
| Type text        | Filter projects in real time                |
| `Up` / `Down`    | Move selection up/down                      |
| `Tab`            | Move selection down                         |
| `Shift+Tab`      | Move selection up                           |
| `Enter`          | Jump to selected project                    |
| `Esc`            | Cancel (exit without jumping)               |
| `Ctrl+C`         | Cancel (exit without jumping)               |
| `Ctrl+U`         | Clear the filter input                      |

### Examples

```bash
# Jump to a project by name
gj my-repo

# Multi-token: project must contain both "backend" and "api"
gj backend api

# Open interactive selector (no pattern)
gj

# Match by alias prefix
gj work api         # "work" is an alias for a domain

# Match by partial name
gj gate             # matches "api-gateway", "gate-service", etc.
```

---

## Current Project (`gj .`)

```
gj .
```

Jumps to the git root of the current repository and loads its configuration. This is useful
when you are already inside a project directory and want to apply the project's config (git
identity, env vars, hooks) without navigating away.

### How It Works

1. Detects the git root by walking up from the current directory until a `.git` directory is
   found.
2. Checks whether the git root is a domain project (under `$_GIT_JUMP_ROOT/<domain>/...`).
3. Loads and merges the appropriate config chain.
4. Outputs shell commands to `cd` to the git root and apply the config.

`gj .` works for both domain projects and non-domain projects. For non-domain projects, config
is loaded by scanning `.git-jump.toml` files from `/` down to the git root.

`gj .` does not require `git-jump setup` to have been run -- it works even without a global
config file.

### Dot Expansion with Other Tokens

When `.` appears alongside other tokens, it is expanded to the current directory name:

```bash
# If cwd is ~/code/git.example.com/backend/api
gj . service        # expands to: gj api service
```

This lets you quickly find sibling projects in the same group.

---

## No-Arg `gj`

```
gj
```

Opens the interactive TUI selector with all projects listed. If you are currently inside a git
repository that is a known domain project, that project is pinned to the top of the list.

This is the fastest way to browse all available projects when you do not have a specific target
in mind.

---

## Clone and Jump (`gjclone`)

```
gjclone <url> [args...]
```

`gjclone` is a shell function (not a binary subcommand) that:

1. Runs `git-jump clone <url>` to clone the repository into the organized directory structure.
2. Runs `gj .` on the cloned directory to jump to it and load its config.

### URL Formats

Both HTTPS and SSH URL formats are supported:

```bash
# HTTPS
gjclone https://github.com/my-org/my-repo

# SSH
gjclone git@github.com:my-org/my-repo.git

# GitLab
gjclone https://gitlab.com/my-org/my-repo
gjclone git@gitlab.com:my-org/my-repo.git
```

The URL is parsed to extract the domain, group path, and project name. The repository is cloned
to `$root/<domain>/<groups>/<project>`.

### Passing Extra Arguments to git clone

Any arguments after the URL are passed through directly to `git clone`:

```bash
# Shallow clone
gjclone https://github.com/my-org/my-repo --depth 1

# Clone specific branch
gjclone https://github.com/my-org/my-repo --branch develop

# Combined options
gjclone https://github.com/my-org/my-repo --depth 1 --single-branch --branch main
```

When using `git-jump` directly, the `--debug` flag must come before the subcommand:

```bash
git-jump --debug clone https://github.com/my-org/my-repo --depth 1
```

### Auto-Generated Domain Config

On the first clone to a new domain, `gj` automatically creates a domain-level `.git-jump.toml`
with pre-filled settings:

- **github.com**, **gitlab.com**, **bitbucket.org**: `web_url_template` is set to the correct
  format for that host.
- **Other domains**: all `web_url_template` options are commented out with examples for each
  format. Edit the file to activate the correct one.

A hint is printed to stderr pointing to the new config file.

### Target Already Exists

If the target directory already exists, `gjclone` skips the clone step and jumps directly to
the existing directory. This makes `gjclone` idempotent -- safe to run multiple times.

### Pre-Configured Domains

| Domain          | Auto-configured `web_url_template`                                          |
|-----------------|-----------------------------------------------------------------------------|
| `github.com`    | `https://{domain}/{groups}/{project}/tree/{branch}/{path}`                  |
| `gitlab.com`    | `https://{domain}/{groups}/{project}/-/tree/{branch}/{path}`                |
| `bitbucket.org` | `https://{domain}/{groups}/{project}/src/{branch}/{path}`                   |

---

## Browse Command

```
git-jump browse [pattern...]
```

Opens the project's web page in a browser. The URL is constructed from the `web_url_template`
config field.

### URL Construction

Priority order:

1. **`web_url_template`** from the merged config chain (domain/group/project `.git-jump.toml`).
2. **Default inference**: `https://{domain}/{groups}/{project}` (domain projects only, no
   template required).
3. **Error**: non-domain projects without a `web_url_template` cannot be browsed.

### Branch and Path Detection

When `{branch}` appears in the template, `gj` runs `git rev-parse --abbrev-ref HEAD` to detect
the current branch. When `{path}` appears, `gj` uses the current working directory relative to
the git root.

This means `git-jump browse` (no args, run from inside a project subdirectory) opens the
browser at the exact file or directory you are currently viewing.

### No-Arg Mode (Current Directory)

```bash
git-jump browse
```

When called without a pattern, `gj` detects the current project from the working directory:

1. Walks up from cwd to find a `.git` directory inside the root.
2. If found and the domain is registered, uses that as the current project.
3. Falls back to non-domain detection (git root anywhere on the filesystem).

### Pattern Mode

```bash
git-jump browse api
git-jump browse work backend
```

Pattern matching follows the same rules as the jump command (multi-token substring, AND logic,
coverage-based scoring). When multiple projects match, the TUI selector opens.

### Browser Configuration

By default, `gj` uses the system default browser (via the `webbrowser` crate). To use a
specific browser, set the `browser` field in the global config:

```toml
# ~/.config/git-jump/config.toml
browser = "firefox --new-tab {url}"
```

The `{url}` placeholder is replaced with the constructed URL.

### Non-Domain Project Support

Non-domain projects can be browsed if a `web_url_template` is set in a `.git-jump.toml` along
the path from `/` to the git root. The template can use static URLs (no placeholders) or any
combination of the available variables.

### Examples

```bash
# Browse current project (opens at current subdirectory if {path} in template)
git-jump browse

# Browse a specific project by pattern
git-jump browse my-repo

# Browse with multi-token pattern
git-jump browse work backend
```

---

## Debug Mode

```
git-jump --debug <subcommand>
```

The `--debug` flag is a global option that prints diagnostic information to stderr. It works
with any subcommand.

### What Debug Output Includes

- Resolved root directory and known domains
- Pattern matching: candidates considered, scores, selected project
- Config chain: which `.git-jump.toml` files were loaded at each level
- Merged config values (git_config, env, hooks, web_url_template)
- Shell commands that will be eval'd
- For browse: URL source (template vs default inference), branch, path, final URL
- For clone: parsed repo URL, target directory, git clone exit code and duration
- Total elapsed time

### Examples

```bash
# Debug a jump
git-jump --debug jump api

# Debug browse URL construction
git-jump --debug browse my-repo

# Debug clone
git-jump --debug clone https://github.com/my-org/my-repo

# Debug current project detection
git-jump --debug jump .
```

Note: `gj --debug` does not work directly because `gj` is a shell function that wraps
`git-jump jump`. Use `git-jump --debug jump <pattern>` instead.

---

## Complete Command Reference

| Command                                  | Description                                              |
|------------------------------------------|----------------------------------------------------------|
| `gj [pattern...]`                        | Jump to matching project (shell function)                |
| `gj .`                                   | Jump to current git project root and load config         |
| `gjclone <url> [args...]`                | Clone repo into organized structure and jump; extra args passed to git clone |
| `git-jump jump [pattern...]`             | Jump (binary subcommand, used by `gj` shell function)    |
| `git-jump browse [pattern...]`           | Open project web page in browser                         |
| `git-jump clone <url> [args...]`         | Clone repo and print target path; extra args passed to git clone |
| `git-jump setup`                         | Interactive first-time configuration wizard              |
| `git-jump init [shell]`                  | Output shell integration script for bash/zsh/fish        |
| `git-jump completions <shell> [partial]` | Generate tab completion candidates                       |
| `git-jump logo [text]`                   | Render text as FIGlet ASCII art                          |
| `git-jump --debug <subcommand>`          | Print debug info to stderr                               |

### Shell Integration Commands

`gj` and `gjclone` are shell functions installed by `git-jump init`. They wrap the binary and
`eval` its stdout to change the working directory and apply environment variables in the parent
shell. This is the same pattern used by [zoxide](https://github.com/ajeetdsouza/zoxide).

The binary itself cannot change the parent shell's cwd or env directly -- it outputs shell
commands to stdout, and the shell function `eval`s them.

### Tab Completion

Tab completion is registered automatically by `git-jump init`. It calls
`git-jump completions <shell> <partial>` to generate candidates, sorted by coverage score.

```bash
gj back<Tab>        # completes to matching project names
gj work <Tab>       # completes projects under the "work" alias
```

---

## Shell Integration Setup

If you have not run `git-jump setup`, add shell integration manually:

**Bash** (`~/.bashrc`):
```bash
eval "$(git-jump init bash)"
```

**Zsh** (`~/.zshrc`):
```zsh
eval "$(git-jump init zsh)"
```

**Fish** (`~/.config/fish/config.fish`):
```fish
git-jump init fish | source
```

After adding the line, reload your shell or open a new terminal.

### Environment Variables

| Variable              | Description                                                  |
|-----------------------|--------------------------------------------------------------|
| `$_GIT_JUMP_ROOT`     | Overrides `root` from global config                          |
| `$_GIT_JUMP_INITIALIZED` | Set by shell integration to prevent double-init           |
| `$_GIT_JUMP_LOGO_TEXT` | Set on each jump; used by shell function to detect env change |
