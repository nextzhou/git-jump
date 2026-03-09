# gj (git-jump)

[![CI](https://img.shields.io/github/actions/workflow/status/nextzhou/git-jump/ci.yml?branch=main&style=flat-square&logo=github&label=CI)](https://github.com/nextzhou/git-jump/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nextzhou/git-jump?style=flat-square&logo=github)](https://github.com/nextzhou/git-jump/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)

A CLI tool to quickly jump between local Git projects with automatic environment setup.

Type `gj <pattern>` to fuzzy-match and jump to a project. On entry, git config, environment
variables, and hooks from the 4-level config hierarchy are applied automatically.

## Features

- Multi-token substring matching with coverage-based scoring + interactive TUI selector
- Organize projects by `domain/group/project` directory structure
- 4-level hierarchical config (global/domain/group/project) with automatic inheritance
- Auto-apply git config, environment variables, and hooks on jump
- Clone repos into the organized structure and auto-load config (`gjclone`)
- Open project web pages in browser with customizable URL templates (`git-jump browse`)
- Shell integration (bash/zsh/fish) with tab completion
- Path aliases for short-form access (e.g. `alias = "work"` -> `gj work api`)
- Current project awareness: `gj .` jumps to git root; no-arg `gj` pins current project to top
- ASCII art logo on environment switch (FIGlet, configurable per domain/group/project)

## Installation

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/nextzhou/git-jump/releases):

| Platform | Target |
|----------|--------|
| macOS (Apple Silicon) | `aarch64-apple-darwin` |
| macOS (Intel) | `x86_64-apple-darwin` |
| Linux (x86_64) | `x86_64-unknown-linux-gnu` |
| Linux (aarch64) | `aarch64-unknown-linux-gnu` |

Place the `git-jump` binary somewhere on your `$PATH`.

### From Source

Requires Rust 1.86+:

```bash
cargo install --git https://github.com/nextzhou/git-jump
```

Or build manually:

```bash
cargo build --release
# Binary at target/release/git-jump
```

## Quick Start

**Step 1**: Run the interactive setup wizard:

```bash
git-jump setup
```

This configures your project root directory (e.g. `~/code`) and writes shell integration to your
rc file automatically.

The global config is written to `~/.config/git-jump/config.toml`. You can edit it later to set a
custom browser command or default logo text (see [docs/configuration.md](docs/configuration.md)).

**Step 2**: Reload your shell (or open a new terminal).

**Step 3**: Clone a project into the organized directory structure:

```bash
gjclone https://github.com/org/repo
```

On the first clone under a new domain, `gj` auto-generates a domain config template at
`$root/<domain>/.git-jump.toml` with sensible defaults (web URL template, git identity
placeholders, etc.). Review and customize it for your environment.

**Step 4**: Jump to a project:

```bash
gj repo             # match by name across all domains
gj api gate         # multi-token AND match
gj                  # open interactive selector
```

## Shell Integration

If you skipped `git-jump setup`, add shell integration manually:

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

Shell integration provides:
- `gj` function -- wraps `git-jump jump`, evals output to change cwd and apply env/config
- `gjclone` function -- clones a repo into the organized structure and jumps to it
- Tab completion for project names
- ASCII art logo on environment switch

## Configuration Overview

`gj` uses a 4-level config hierarchy. Place `.git-jump.toml` at any directory level to configure
all projects beneath it:

```
~/.config/git-jump/config.toml          # global: root, browser, logo_text
$root/<domain>/.git-jump.toml           # domain: alias, git_config, env, hooks, web_url_template
$root/<domain>/<group>/.git-jump.toml   # group: same fields
$root/<domain>/.../<project>/.git-jump.toml  # project: same fields, highest priority
```

Example domain config (`$root/git.example.com/.git-jump.toml`):

```toml
alias = "work"
web_url_template = "https://{domain}/{groups}/{project}/-/tree/{branch}/{path}"

[git_config]
"user.name" = "Your Name"
"user.email" = "you@company.com"

[hooks]
on_enter = ["echo 'Switched to work environment'"]
```

See [docs/configuration.md](docs/configuration.md) for the full field reference, merge rules,
alias behavior, and URL template variables.

## Command Reference

| Command | Description |
|---------|-------------|
| `gj <pattern...>` | Jump to matching project |
| `gj .` | Jump to current git project root and load config |
| `gjclone <url> [args...]` | Clone repo into organized structure and jump; extra args passed to git clone |
| `git-jump browse [pattern...]` | Open project web page in browser |
| `git-jump setup` | Interactive first-time configuration wizard |
| `git-jump init [shell]` | Output shell integration script |
| `git-jump completions <shell> [partial]` | Generate tab completion candidates |
| `git-jump logo [text]` | Render text as ASCII art (FIGlet) |
| `git-jump --debug <subcommand>` | Print debug info to stderr |

See [docs/usage.md](docs/usage.md) for detailed usage, matching rules, and examples.

## License

MIT
