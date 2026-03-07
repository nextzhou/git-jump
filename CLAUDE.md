# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`gj` (git-jump) is a Rust CLI tool for quickly jumping between local Git projects with automatic environment setup. It follows the zoxide pattern: the Rust binary outputs shell commands to stdout, and a shell wrapper function `eval`s them to change cwd/env in the parent shell.

All three milestones (M1/M2/M3) are implemented:
- **M1**: Core jump + interactive selection + shell function (bash/zsh/fish)
- **M2**: Clone command + 4-level config inheritance + env/hooks
- **M3**: Tab completion + init logo + error handling

## Commands

```bash
# Rust
cargo build                       # Debug build
cargo build --release             # Release build (LTO + strip)
cargo test                        # Run all tests
cargo test <test_name>            # Run a single test
cargo fmt -- --check              # Check formatting
cargo clippy -- -D warnings       # Lint (warnings = errors)

# Shell scripts (shell/*.sh, shell/*.bash)
shellcheck shell/*.bash           # Lint bash scripts
shfmt -d -i 2 -ci -bn shell/*.bash  # Check formatting (diff mode)
shfmt -w -i 2 -ci -bn shell/*.bash  # Auto-format in place
```

## Project Structure

```
gj/
+-- src/main.rs           # Single entry point, clap CLI definition + command routing
+-- src/setup.rs          # Interactive first-time config wizard (dialoguer prompts + validation)
+-- shell/                # Shell integration scripts (embedded into binary via include_str!)
|   +-- gj.bash           # Bash: gj() wrapper + tab completion + env switch logo
|   +-- gj.zsh            # Zsh: gj() wrapper + tab completion + env switch logo
|   +-- gj.fish           # Fish: gj function + tab completion + env switch logo
+-- tests/                # Integration tests via assert_cmd, split by command/feature
|   +-- common/mod.rs     # Shared test helpers (setup_project_root, etc.)
|   +-- cli_basic_test.rs # CLI smoke tests (help, version, usage)
|   +-- init_test.rs      # init command
|   +-- jump_test.rs      # jump command
|   +-- clone_test.rs     # clone command
|   +-- completions_test.rs # completions command
|   +-- debug_test.rs     # --debug flag
|   +-- scoring_test.rs   # Match scoring/ranking (AC-1~AC-14)
|   +-- browse_test.rs    # browse command
|   +-- dot_jump_test.rs  # gj . (current project awareness)
|   +-- alias_test.rs     # Path alias feature (AC-1~AC-21)
|   +-- logo_test.rs      # logo subcommand
+-- docs/
|   +-- configuration.md  # Configuration reference: fields, merge rules, aliases, URL templates
|   +-- usage.md          # Usage guide: jump, clone, browse, debug commands
+-- Cargo.toml            # Edition 2024, MSRV 1.86, deps: clap/serde/toml/dirs/glob/dialoguer/crossterm/ratatui/figlet-rs
+-- clippy.toml           # MSRV 1.86
+-- rustfmt.toml          # max_width=100, field_init_shorthand
+-- lefthook.yml          # Pre-commit: fmt+clippy+shellcheck+shfmt; commit-msg: Conventional Commits
+-- .github/
|   +-- workflows/
|   |   +-- ci.yml        # CI: check(fmt+clippy+shellcheck+shfmt) -> test -> build(main/tags)
|   |   +-- release.yml   # Release: multi-platform builds (macOS x86_64/arm64, Linux x86_64/aarch64)
+-- LICENSE               # MIT license
+-- CONTRIBUTING.md       # Contribution guide
+-- CHANGELOG.md          # Version history
+-- CLAUDE.md             # This file: dev guide, architecture, conventions, commands
```

### Module Structure

```
src/
  main.rs          # CLI definition (clap derive) + command routing
  error.rs         # Error types (Error enum + Result alias)
  config.rs        # Config loading/merging (global + 4-level local hierarchy) + alias validation
  project.rs       # Project discovery (dir walking) + pattern matching (glob) + alias loading
  filter.rs        # Multi-token substring matching with highlight ranges for interactive filter
  score.rs         # Match scoring: coverage-based ranking (project_score, group_score)
  select.rs        # Interactive selection: crossterm + ratatui TUI with real-time filtering + scoring
  jump.rs          # Jump command: find project, merge config, output shell commands
  browse.rs        # Browse command: construct URL from template, detect branch/path, open browser
  clone.rs         # Clone command: parse repo URL (https:// or git@), git clone, output target path, auto-generate domain config
  completions.rs   # Tab completion: output matching project names (sorted by score)
  resolve.rs       # Project resolution: pattern matching + interactive selection + alias display candidates + collision detection
  shell.rs         # Shell integration: embed scripts via include_str!
  setup.rs         # Interactive first-time config wizard (dialoguer prompts + validation)
  debug.rs         # Debug logging: buffered stderr output with path abbreviation
```

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| CLI definition & commands | `src/main.rs` | clap derive: `Cli` struct + `Commands` enum |
| Interactive filter/select | `src/select.rs` + `src/filter.rs` + `src/score.rs` | crossterm+ratatui TUI, multi-token filter, coverage-based scoring |
| Interactive setup wizard | `src/setup.rs` | dialoguer prompts + validation + config writing |
| Shell integration scripts | `shell/` | `.bash`/`.zsh`/`.fish` files, embedded via `include_str!` |
| Configuration reference | `docs/configuration.md` | Config fields, merge rules, aliases, URL templates |
| Usage guide | `docs/usage.md` | Jump, clone, browse, debug commands with examples |
| Integration tests | `tests/*_test.rs` | Split by command; shared helpers in `tests/common/mod.rs` |
| CI pipeline | `.github/workflows/` | check/test/build + multi-platform release builds |
| Git hooks | `lefthook.yml` | Parallel fmt+clippy+shellcheck+shfmt on pre-commit |

## Architecture

### Shell Integration (zoxide-style)

The Rust binary **cannot** change the parent shell's cwd or env directly. Instead:
1. Binary computes target path, env vars, git config, hooks
2. Outputs shell commands to stdout (`cd /path`, `export VAR=val`, `git config ...`)
3. Shell function wraps binary and `eval`s stdout
4. Tab completion: binary computes candidates, shell registers completion function
5. `gjclone` shell function combines `git-jump clone` (outputs target path) + `gj .` (loads config)

### Config Hierarchy (4 levels, TOML)

```
~/.config/git-jump/config.toml                    # Global: root, browser, logo_text
  $_GIT_JUMP_ROOT/<domain>/.git-jump.toml         # Domain: alias, web_url_template, git_config, env, hooks, logo_text
    .../<group>/.git-jump.toml                     # Group: alias, web_url_template, git_config, env, hooks, logo_text
      .../<project>/.git-jump.toml                 # Project: alias, web_url_template, git_config, env, hooks, logo_text
```

Merge rules:
- `env` / `git_config`: same key = child overrides, different keys = merge
- `hooks.on_enter`: append mode, all levels execute parent-to-child order
- `web_url_template` / `logo_text`: child overrides parent
- `alias`: NOT part of merge chain; each project takes its nearest ancestor's alias

Config path resolution: `$XDG_CONFIG_HOME/git-jump/` or `~/.config/git-jump/` (XDG spec).

### Shell Scripts (`shell/`)

Shell integration scripts that `gj init <shell>` outputs. Stored as standalone files for proper linting, embedded into the binary at compile time via `include_str!`.

Key constraints:
- Must not introduce perceptible shell startup latency (< 10ms budget)
- The `gj()` function must `eval` the binary's stdout to change cwd/env in the parent shell
- Must handle missing `gj` binary gracefully (clear error message)
- Must support `$_GIT_JUMP_ROOT` and `$_GIT_JUMP_INITIALIZED` environment variables
- Tab completion registered via `complete -F` (bash), `compdef` (zsh), `complete -c` (fish)

### CLI Structure (`src/main.rs`)

Uses clap derive API:
- `gj <pattern>` - jump to matching project
- `gjclone <repo>` - clone, cd, and load config (shell function combining `git-jump clone` + `gj .`)
- `git-jump clone <repo>` - clone a full URL (https:// or git@) and output target path to stdout
- `gj init <shell>` - output shell integration script
- `gj completions <shell> [partial]` - generate tab completions
- `gj setup` - interactive first-time configuration wizard

## Conventions

### Rust
- **Rust edition 2024**, MSRV 1.86
- **rustfmt**: max_width=100, use_field_init_shorthand=true
- **clippy**: `-D warnings` (zero warnings policy)
- Integration tests in `tests/` using `assert_cmd` + `tempfile`; unit tests as `#[cfg(test)]` modules
- Performance budgets: shell startup < 10ms, single-match jump < 50ms

### Shell Scripts
- Shell integration scripts live in `shell/` directory
- File naming: `gj.bash` (bash), `gj.zsh` (zsh), `gj.fish` (fish)
- Embedded into the Rust binary at compile time via `include_str!("../shell/gj.bash")`
- **shellcheck**: all `.sh`/`.bash` files must pass with zero warnings
- **shfmt**: 2-space indent, case body indent, binary ops at start of next line (`-i 2 -ci -bn`)
- shellcheck/shfmt only apply to `.sh`/`.bash` files (no tooling for zsh/fish -- rely on manual review and testing)
- For zsh scripts: follow bash conventions where possible, test on zsh >= 5.0
- For fish scripts: follow fish idioms (no POSIX compatibility needed), test on fish >= 3.0
- All shell functions must be POSIX-safe in variable quoting (always double-quote `$variables`)

### Git
- **Conventional Commits** enforced by lefthook: `<type>(<scope>): <description>`
  - Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert

### CI Pipeline
- `cargo fmt -- --check` + `cargo clippy -- -D warnings` (check stage)
- `shellcheck` + `shfmt -d` for `shell/*.{sh,bash}` (check stage, runs only when shell files change)
- `cargo test` (test stage)
- `cargo build --release` (build stage, main/tags only)
- Artifacts: `target/release/git-jump`

### Testing

**Integration tests** (`tests/*_test.rs`):
- One file per command/feature, using `assert_cmd` + `tempfile`
- Shared helpers in `tests/common/mod.rs` (each test file: `mod common;`)
- New command/feature = new `tests/<feature>_test.rs` file; do NOT pile into existing files
- Feature-specific helpers stay local to their test file; cross-file helpers go in `common/mod.rs`

**Unit tests** (`#[cfg(test)]` modules in `src/*.rs`):
- Co-located with source: each `src/<module>.rs` has a `#[cfg(test)] mod tests` at the bottom
- Test pure logic only (parsing, scoring, filtering, URL construction, etc.)
- Use `TempDir` for any test needing filesystem; never touch real user paths
- Prefer testing public functions; test private helpers only when logic is non-trivial

## Anti-Patterns

- Do NOT use emoji in code or documentation
- Do NOT use `rm` -- use `trash` command instead
- Do NOT attempt to change parent shell cwd directly from Rust binary (use zoxide pattern: output shell commands to stdout for `eval`)
- Do NOT introduce perceptible latency in shell startup (< 10ms budget)

## Notes

- `Cargo.lock` is committed (binary crate convention -- ensures reproducible builds)
- Release profile: LTO + strip enabled
- Target platforms: macOS + Linux

## Documentation Maintenance

After each development session, check and update relevant docs (docs/configuration.md, docs/usage.md, this file) to keep them in sync with code changes.

### README Maintenance

When CLI commands, config fields, or shell integration behavior changes, update `README.md` accordingly. README focuses on user-facing configuration and usage; keep feature descriptions concise, configuration examples detailed.
