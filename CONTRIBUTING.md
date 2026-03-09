# Contributing to git-jump

## Development Setup

Requires Rust 1.86+.

```bash
git clone https://github.com/nextzhou/git-jump
cd git-jump
cargo build
```

Install [lefthook](https://github.com/evilmartians/lefthook) for pre-commit hooks:

```bash
lefthook install
```

## Running Tests

```bash
cargo test          # run all tests
cargo test <name>   # run a specific test
```

## Code Style

```bash
cargo fmt -- --check    # check formatting
cargo fmt               # auto-format
cargo clippy -- -D warnings  # lint (zero warnings policy)
```

Shell scripts (`shell/*.bash`):

```bash
shellcheck shell/*.bash
shfmt -d -i 2 -ci -bn shell/*.bash
```

## Commit Messages

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`

## Architecture

See [CLAUDE.md](CLAUDE.md) for a detailed description of the project architecture, module
structure, shell integration design, and config hierarchy.

## Pull Requests

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make changes, ensure all checks pass
4. Submit a pull request with a clear description of the change
