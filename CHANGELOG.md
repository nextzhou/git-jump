# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-03-09

### Added

- `gjclone` now supports passing extra arguments to `git clone` (e.g. `gjclone <url> -- --depth 1`)
- `--version` output now auto-detects version from git tags at compile time via `build.rs`

### Changed

- Release workflow fetches git tags during checkout to ensure correct version in CI builds

## [1.0.0] - 2024-01-01

Initial public release.

### Added

- Multi-token substring matching with coverage-based scoring
- Interactive TUI selector (crossterm + ratatui) with real-time filtering
- Project organization by `domain/group/project` directory structure
- 4-level hierarchical config (global/domain/group/project) with automatic inheritance
- Auto-apply git config, environment variables, and hooks on jump
- Clone repos into organized structure with auto-config generation (`gjclone`)
- Browser integration with customizable URL templates (`git-jump browse`)
- Shell integration for bash, zsh, and fish with tab completion
- Path aliases for short-form project access
- Current project awareness: `gj .` jumps to git root
- ASCII art logo on environment switch (FIGlet)
- Interactive first-time setup wizard (`git-jump setup`)
- Debug mode (`git-jump --debug`) for troubleshooting
