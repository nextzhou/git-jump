# Configuration Reference

`gj` (git-jump) uses a 4-level hierarchical configuration system. Settings defined at a higher
level (closer to the root) are inherited by all projects beneath them, and child configs can
override or extend parent settings.

## Overview

```
~/.config/git-jump/config.toml                    # Global
$root/<domain>/.git-jump.toml                     # Domain
$root/<domain>/<group>/.git-jump.toml             # Group
$root/<domain>/.../<project>/.git-jump.toml       # Project
```

The **global config** holds machine-wide settings (root path, browser command, default logo).
The **domain/group/project configs** (`.git-jump.toml`) hold per-environment settings that are
merged as you descend the directory tree.

---

## Global Configuration

**File**: `~/.config/git-jump/config.toml`

`gj` follows the XDG Base Directory Specification. If `$XDG_CONFIG_HOME` is set, the config
directory is `$XDG_CONFIG_HOME/git-jump/`; otherwise it defaults to `~/.config/git-jump/`.

Run `git-jump setup` to create this file interactively.

### Fields

#### `root`

Type: `string` (path)
Required: yes (or set `$_GIT_JUMP_ROOT`)

The root directory that contains all your organized Git projects. All domain directories live
directly under this path.

```toml
root = "~/code"
```

The `$_GIT_JUMP_ROOT` environment variable takes precedence over this field when set.

#### `browser`

Type: `string` (shell command template)
Required: no

Custom browser command for `git-jump browse`. Use `{url}` as a placeholder for the target URL.
When not set, `gj` uses the system default browser.

```toml
browser = "firefox --new-tab {url}"
```

```toml
browser = "open -a 'Google Chrome' {url}"
```

#### `logo_text`

Type: `string`
Required: no

Default ASCII art logo text rendered via FIGlet when switching environments. This is the
lowest-priority fallback; domain/group/project configs override it.

```toml
logo_text = "dev"
```

---

## Directory Structure

Projects are organized under the root directory in a `domain/group/project` hierarchy:

```
~/code/                                    # root
  github.com/                              # domain
    .git-jump.toml                         # domain config
    my-org/                                # group
      .git-jump.toml                       # group config (optional)
      backend-api/                         # project
        .git-jump.toml                     # project config (optional)
        .git/
      frontend-app/
        .git/
    another-org/
      data-pipeline/
        .git/
  git.example.com/                         # another domain
    .git-jump.toml
    devops/
      helm-charts/
        .git/
      terraform/
        .git/
```

The `domain` component is the Git server hostname (e.g. `github.com`, `git.example.com`).
The `group` component maps to the organization or namespace on the server.
Subgroups are supported: `domain/org/subgroup/project` is a valid 4-component path.

---

## Hierarchical Config Files (`.git-jump.toml`)

Place a `.git-jump.toml` file at any directory level (domain, group, subgroup, or project) to
configure all projects beneath it. The file uses TOML syntax.

### Supported Fields by Level

| Field              | Global | Domain | Group | Project |
|--------------------|--------|--------|-------|---------|
| `root`             | yes    | --     | --    | --      |
| `browser`          | yes    | --     | --    | --      |
| `logo_text`        | yes    | yes    | yes   | yes     |
| `alias`            | --     | yes    | yes   | yes     |
| `web_url_template` | --     | yes    | yes   | yes     |
| `git_config`       | --     | yes    | yes   | yes     |
| `env`              | --     | yes    | yes   | yes     |
| `hooks.on_enter`   | --     | yes    | yes   | yes     |

---

## Config Fields Reference

### `alias`

Type: `string`
Levels: domain, group, project

A short identifier for the directory. Aliases create an alternative path prefix for matching,
tab completion, and the TUI selector.

```toml
alias = "work"
```

Constraints:
- Must not be empty
- Must not contain `/` or whitespace

With `alias = "work"` on a domain directory, the path `git.example.com/backend/api` becomes
accessible as `work/backend/api` in addition to its full path.

See [Path Aliases](#path-aliases) for detailed behavior.

### `web_url_template`

Type: `string` (URL template)
Levels: domain, group, project

URL template used by `git-jump browse` to construct the project web page URL. Supports
placeholder variables that are substituted at runtime.

```toml
# GitHub
web_url_template = "https://{domain}/{groups}/{project}/tree/{branch}/{path}"

# GitLab
web_url_template = "https://{domain}/{groups}/{project}/-/tree/{branch}/{path}"

# Bitbucket
web_url_template = "https://{domain}/{groups}/{project}/src/{branch}/{path}"

# Custom server with non-standard port
web_url_template = "https://{domain}:8443/{groups}/{project}"

# Project home page only (no branch/path)
web_url_template = "https://{domain}/{groups}/{project}"
```

See [URL Template Variables](#url-template-variables) for the full variable reference.

**Merge rule**: child overrides parent.

### `git_config`

Type: `table` (key-value pairs)
Levels: domain, group, project

Git configuration key-value pairs applied automatically via `git config` each time you jump to
a project. Useful for setting per-domain identity (name, email, signing key).

```toml
[git_config]
"user.name" = "Your Name"
"user.email" = "you@company.com"
"commit.gpgsign" = "true"
"user.signingkey" = "ABCDEF1234567890"
```

Note: dotted keys (e.g. `user.name`) must be quoted in TOML.

**Merge rule**: same key -- child overrides parent; different keys -- merged from all levels.

### `env`

Type: `table` (key-value pairs)
Levels: domain, group, project

Environment variables exported into the shell each time you jump to a project.

```toml
[env]
GOPATH = "/home/you/go"
KUBECONFIG = "/home/you/.kube/work-config"
AWS_PROFILE = "work"
```

**Merge rule**: same key -- child overrides parent; different keys -- merged from all levels.

### `hooks.on_enter`

Type: `array of strings` (shell commands)
Levels: domain, group, project

Shell commands executed in the parent shell each time you jump to a project. Commands run in
parent-to-child order (domain hooks first, then group, then project).

```toml
[hooks]
on_enter = [
  "echo 'Switched to work environment'",
  "source ~/.work-aliases",
]
```

**Merge rule**: append mode -- all levels execute, parent-to-child order.

### `logo_text`

Type: `string`
Levels: global, domain, group, project

Text rendered as FIGlet ASCII art when switching to an environment with a different `logo_text`
value. Set to an empty string to suppress the logo for a specific subtree.

```toml
logo_text = "Work"
```

**Merge rule**: child overrides parent. `None` (field absent) inherits from parent; an explicit
empty string `""` suppresses the logo.

---

## Merge Rules

When you jump to a project, `gj` walks from the domain directory down to the project directory,
loading `.git-jump.toml` at each level and merging them in order:

```
domain config -> group config -> subgroup config -> project config
```

The merge rules for each field type:

| Field              | Rule                                                        |
|--------------------|-------------------------------------------------------------|
| `env`              | Same key: child overrides. Different keys: merged.          |
| `git_config`       | Same key: child overrides. Different keys: merged.          |
| `hooks.on_enter`   | Append: all levels execute, parent-to-child order.          |
| `web_url_template` | Child overrides parent entirely.                            |
| `logo_text`        | Child overrides parent. Absent (`None`) inherits.           |
| `alias`            | Not merged. Each project uses its nearest ancestor's alias. |

### Example

Given this structure:

```
git.example.com/.git-jump.toml       # alias="work", user.email="team@example.com"
  backend/.git-jump.toml             # user.email="backend@example.com", GOPATH="/go"
    api/.git-jump.toml               # on_enter=["make deps"]
```

Jumping to `api` produces:
- `alias`: `work` (from domain, not merged)
- `user.email`: `backend@example.com` (group overrides domain)
- `GOPATH`: `/go` (from group, no conflict)
- `on_enter`: `["make deps"]` (only project level defined hooks here)

---

## Path Aliases

The `alias` field creates a short name for a directory, making it usable as a path prefix in
matching, tab completion, and the TUI selector.

### Single-Level Alias

With `alias = "work"` in `git.example.com/.git-jump.toml`:

```
git.example.com/backend/api   ->   work/backend/api
git.example.com/frontend/app  ->   work/frontend/app
```

Both the original path and the alias path are valid for matching:

```bash
gj work api       # matches via alias prefix
gj example api    # matches via domain name
gj api            # matches by project name alone
```

### Multi-Level Aliases

Aliases can be set at group level too. With `alias = "be"` in
`git.example.com/backend/.git-jump.toml`:

```
git.example.com/backend/api   ->   work/be/api   (domain alias + group alias)
```

### Cross-Domain Aggregation

When multiple domains have the same alias, all their projects appear under that alias prefix in
the TUI selector. This lets you group projects from different servers under a single short name.

### Collision Disambiguation

If two projects resolve to the same alias path, `gj` detects the collision and shows both
candidates in the TUI selector with their full paths for disambiguation.

### Alias Constraints

- Must not be empty
- Must not contain `/` (single path segment only)
- Must not contain whitespace

---

## URL Template Variables

The following variables are available in `web_url_template`:

| Variable    | Description                                                        | Example value         |
|-------------|--------------------------------------------------------------------|-----------------------|
| `{domain}`  | Git server hostname                                                | `github.com`          |
| `{groups}`  | Group path, `/`-separated, including subgroups                     | `my-org/sub`          |
| `{project}` | Project (repository) name                                          | `backend-api`         |
| `{branch}`  | Current local Git branch (runs `git rev-parse --abbrev-ref HEAD`)  | `main`                |
| `{path}`    | Current working directory relative to the git root                 | `src/handlers`        |

Notes:
- `{branch}` triggers a `git` subprocess only when present in the template.
- `{path}` is empty when you are at the project root or when using pattern-based browse.
- When `{branch}` is `HEAD` (detached HEAD state), `gj` prints a warning.
- Consecutive slashes in the rendered URL are collapsed automatically.

### Pre-configured Templates

`gjclone` automatically creates a domain config with the correct template for known hosts:

| Host            | Template                                                                    |
|-----------------|-----------------------------------------------------------------------------|
| `github.com`    | `https://{domain}/{groups}/{project}/tree/{branch}/{path}`                  |
| `gitlab.com`    | `https://{domain}/{groups}/{project}/-/tree/{branch}/{path}`                |
| `bitbucket.org` | `https://{domain}/{groups}/{project}/src/{branch}/{path}`                   |
| Other           | All `web_url_template` lines commented out -- fill in manually              |

---

## Non-Domain Projects

Projects that live outside the organized `root/domain/group/project` structure are called
non-domain projects. `gj` supports them with reduced functionality:

- `gj .` works: detects the git root and loads config by walking from `/` to the git root.
- `git-jump browse` works if a `web_url_template` is set in a `.git-jump.toml` along the path.
- Pattern-based `gj <pattern>` does not discover non-domain projects.

Config scanning for non-domain projects walks every ancestor directory from `/` to the git root,
loading `.git-jump.toml` at each level. Permission errors are silently skipped.

---

## Examples

### Domain-Level Config

`~/code/git.example.com/.git-jump.toml`:

```toml
# Short alias for this domain
alias = "work"

# Browse URL template (GitLab format)
web_url_template = "https://{domain}/{groups}/{project}/-/tree/{branch}/{path}"

# ASCII art logo shown when switching to this environment
logo_text = "Work"

# Git identity applied to all projects under this domain
[git_config]
"user.name" = "Your Name"
"user.email" = "you@company.com"

# Environment variables
[env]
KUBECONFIG = "/home/you/.kube/work-config"

# Hooks run on every jump into this domain
[hooks]
on_enter = ["echo 'Switched to work environment'"]
```

### Group-Level Config

`~/code/git.example.com/devops/.git-jump.toml`:

```toml
# Short alias for this group (stacks with domain alias)
alias = "ops"

# Override browse URL for this group (different path format)
web_url_template = "https://{domain}/{groups}/{project}/browse/{branch}/{path}"

# Additional env vars for devops projects
[env]
TERRAFORM_ENV = "production"

# Hooks appended after domain-level hooks
[hooks]
on_enter = ["source ~/.devops-aliases"]
```

### Project-Level Config

`~/code/git.example.com/devops/helm-charts/.git-jump.toml`:

```toml
# Override git identity for this specific project
[git_config]
"user.email" = "charts-maintainer@example.com"

# Project-specific env
[env]
HELM_NAMESPACE = "production"

# Project-specific hook (runs after domain and group hooks)
[hooks]
on_enter = ["helm repo update"]
```

### Global Config

`~/.config/git-jump/config.toml`:

```toml
root = "~/code"
browser = "firefox --new-tab {url}"
logo_text = "dev"
```
