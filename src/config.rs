use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Top-level global configuration.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct GlobalConfig {
    /// Root directory for all projects (`$_GIT_JUMP_ROOT`).
    pub root: Option<String>,
    /// Browser command template for `browse` (e.g., `"firefox --new-tab {url}"`).
    pub browser: Option<String>,
    /// Text to render as FIGlet ASCII art logo.
    pub logo_text: Option<String>,
}

/// Per-directory configuration found in `.git-jump.toml`.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct LocalConfig {
    /// Path alias for this directory (single-segment identifier).
    pub alias: Option<String>,
    /// URL template for `browse` (e.g., `"https://{domain}:8443/{groups}/{project}"`).
    pub web_url_template: Option<String>,
    /// Git config key-value pairs (e.g., `user.name = "Alice"`).
    pub git_config: Option<BTreeMap<String, String>>,
    /// Environment variable key-value pairs.
    pub env: Option<BTreeMap<String, String>>,
    /// Hook definitions.
    pub hooks: Option<Hooks>,
    /// Text to render as FIGlet ASCII art logo (child overrides parent).
    pub logo_text: Option<String>,
}

/// Hook scripts executed on project entry.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Hooks {
    /// Commands to run when entering a project directory.
    pub on_enter: Vec<String>,
}

/// Fully merged configuration for a concrete project path.
#[derive(Debug, Default)]
pub struct MergedConfig {
    pub web_url_template: Option<String>,
    pub git_config: BTreeMap<String, String>,
    pub env: BTreeMap<String, String>,
    /// All on_enter hooks in parent-to-child order.
    pub on_enter_hooks: Vec<String>,
    /// FIGlet logo text (after merge chain, before global fallback).
    pub logo_text: Option<String>,
}

impl MergedConfig {
    /// Layer a child `LocalConfig` on top of the current merged state.
    ///
    /// - `git_config` / `env`: child overrides same key, different keys merge.
    /// - `hooks.on_enter`: append (parent-to-child ordering).
    /// - `web_url_template` / `logo_text`: child overrides parent.
    pub fn apply(&mut self, child: &LocalConfig) {
        if let Some(tmpl) = &child.web_url_template {
            self.web_url_template = Some(tmpl.clone());
        }
        if let Some(gc) = &child.git_config {
            for (k, v) in gc {
                self.git_config.insert(k.clone(), v.clone());
            }
        }
        if let Some(env) = &child.env {
            for (k, v) in env {
                self.env.insert(k.clone(), v.clone());
            }
        }
        if let Some(hooks) = &child.hooks {
            self.on_enter_hooks.extend(hooks.on_enter.iter().cloned());
        }
        if let Some(text) = &child.logo_text {
            self.logo_text = Some(text.clone());
        }
    }
}

/// Path to the known-domains registry file (`~/.config/git-jump/domains`).
pub fn domains_file_path() -> Result<PathBuf> {
    Ok(global_config_dir()?.join("domains"))
}

/// Load known domains from `~/.config/git-jump/domains`.
///
/// Each non-empty, non-comment line is a domain name.
/// Returns an empty vec if the file does not exist.
pub fn load_known_domains() -> Result<Vec<String>> {
    let path = domains_file_path()?;
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::Io {
            source: e,
            path: Some(path),
        }),
    }
}

/// Register a domain in the known-domains file, deduplicating the entire list.
pub fn register_domain(domain: &str) -> Result<()> {
    let mut domains = load_known_domains()?;
    domains.push(domain.to_string());
    dedup_stable(&mut domains);
    write_domains(&domains)
}

fn dedup_stable(domains: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    domains.retain(|d| seen.insert(d.clone()));
}

fn write_domains(domains: &[String]) -> Result<()> {
    let path = domains_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Io {
            source: e,
            path: Some(parent.to_path_buf()),
        })?;
    }
    let content = domains
        .iter()
        .map(|d| d.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, content + "\n").map_err(|e| Error::Io {
        source: e,
        path: Some(path),
    })?;
    Ok(())
}

pub fn global_config_dir() -> Result<PathBuf> {
    // XDG spec: $XDG_CONFIG_HOME/git-jump or ~/.config/git-jump
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg).join("git-jump"));
    }
    dirs::home_dir()
        .map(|h| h.join(".config/git-jump"))
        .ok_or_else(|| Error::Config("cannot determine home directory".into()))
}

/// Full path to the global config file (`~/.config/git-jump/config.toml`).
pub fn global_config_path() -> Result<PathBuf> {
    Ok(global_config_dir()?.join("config.toml"))
}

pub fn config_file_exists() -> bool {
    global_config_path().map(|p| p.exists()).unwrap_or(false)
}

/// Load the global config from `~/.config/git-jump/config.toml`.
///
/// Returns `Default` when the file does not exist (first run).
pub fn load_global_config() -> Result<GlobalConfig> {
    let path = global_config_path()?;
    load_toml_or_default(&path)
}

/// Resolve the project root directory.
///
/// Priority: `$_GIT_JUMP_ROOT` env var > `config.root` field.
pub fn resolve_root(global: &GlobalConfig) -> Result<PathBuf> {
    if let Ok(env_root) = std::env::var("_GIT_JUMP_ROOT") {
        let p = PathBuf::from(env_root);
        return if p.is_dir() {
            Ok(p)
        } else {
            Err(Error::RootNotFound { path: p })
        };
    }
    if let Some(root) = &global.root {
        let p = expand_tilde(root);
        return if p.is_dir() {
            Ok(p)
        } else {
            Err(Error::RootNotFound { path: p })
        };
    }
    Err(Error::Config(
        "'root' is not set in config and $_GIT_JUMP_ROOT is not defined".into(),
    ))
}

/// Load a `.git-jump.toml` from the given directory, returning `Default` if absent.
pub fn load_local_config(dir: &Path) -> Result<LocalConfig> {
    let path = dir.join(".git-jump.toml");
    load_toml_or_default(&path)
}

/// Collect and merge configs from root down to the project directory.
///
/// Walks from `root/<domain>/` through each path component to the project,
/// loading `.git-jump.toml` at each level and merging parent-to-child.
pub fn collect_merged_config(root: &Path, project_path: &Path) -> Result<MergedConfig> {
    let relative = project_path
        .strip_prefix(root)
        .map_err(|_| Error::Config("project path is not under root".into()))?;

    let mut merged = MergedConfig::default();
    let mut current = root.to_path_buf();

    for component in relative.components() {
        current.push(component);
        let local = load_local_config(&current)?;
        merged.apply(&local);
    }

    Ok(merged)
}

/// Collect configs for non-domain projects: walk from `/` to `git_root`,
/// loading `.git-jump.toml` at each level. Permission errors are silently skipped.
pub fn collect_merged_config_non_domain(git_root: &Path) -> MergedConfig {
    let mut merged = MergedConfig::default();

    let mut dirs: Vec<&Path> = git_root.ancestors().collect();
    dirs.reverse();

    for dir in dirs {
        let toml_path = dir.join(".git-jump.toml");
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            if let Ok(local) = toml::from_str::<LocalConfig>(&content) {
                merged.apply(&local);
            }
        }
    }

    merged
}

fn load_toml_or_default<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(toml::from_str(&content)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(Error::Io {
            source: e,
            path: Some(path.to_path_buf()),
        }),
    }
}

pub fn validate_alias(value: &str) -> bool {
    !value.is_empty() && !value.contains('/') && !value.chars().any(|c| c.is_whitespace())
}

enum DomainKind {
    GitHub,
    GitLab,
    Bitbucket,
    Unknown,
}

fn classify_domain(domain: &str) -> DomainKind {
    match domain {
        "github.com" => DomainKind::GitHub,
        "gitlab.com" => DomainKind::GitLab,
        "bitbucket.org" => DomainKind::Bitbucket,
        _ => DomainKind::Unknown,
    }
}

/// Check whether a domain-level `.git-jump.toml` exists; create one if missing.
///
/// Returns `Ok(Some(path))` when a new file was created, `Ok(None)` when
/// the config already existed.  Write failures are propagated as `Err`.
pub fn ensure_domain_config(
    root: &Path,
    domain: &str,
    dbg: &mut crate::debug::DebugLog,
) -> Result<Option<PathBuf>> {
    let domain_dir = root.join(domain);
    let config_path = domain_dir.join(".git-jump.toml");
    let kind = classify_domain(domain);

    if dbg.is_enabled() {
        let kind_label = match &kind {
            DomainKind::GitHub => "github",
            DomainKind::GitLab => "gitlab",
            DomainKind::Bitbucket => "bitbucket",
            DomainKind::Unknown => "unknown",
        };
        dbg.log(&format!(
            "domain config: {}",
            crate::debug::abbreviate_path(&config_path)
        ));
        dbg.log_indent(&format!("exists: {}", config_path.exists()));
        dbg.log_indent(&format!("domain kind: {kind_label}"));
    }

    if config_path.exists() {
        return Ok(None);
    }

    let content = format_domain_config(domain);
    std::fs::write(&config_path, &content).map_err(|e| Error::Io {
        source: e,
        path: Some(config_path.clone()),
    })?;

    if dbg.is_enabled() {
        dbg.log("created domain config");
    }

    Ok(Some(config_path))
}

fn format_domain_config(domain: &str) -> String {
    let kind = classify_domain(domain);

    let alias_example = match kind {
        DomainKind::GitHub => "\"gh\"",
        DomainKind::GitLab => "\"gl\"",
        DomainKind::Bitbucket => "\"bb\"",
        DomainKind::Unknown => "\"myalias\"",
    };

    let web_url_section = match kind {
        DomainKind::GitHub => format!(
            "\
# Browse URL template -- used by `git-jump browse`.
# Available variables:
#   {{domain}}  -- Git server domain ({domain})
#   {{groups}}  -- group path including subgroups, \"/\" separated (org, org/sub)
#   {{project}} -- project name (my-repo)
#   {{branch}}  -- current local Git branch; git command runs only when used in template
#   {{path}}    -- cwd relative to git root; empty when not in a subdirectory
web_url_template = \"https://{{domain}}/{{groups}}/{{project}}/tree/{{branch}}/{{path}}\""
        ),
        DomainKind::GitLab => format!(
            "\
# Browse URL template -- used by `git-jump browse`.
# Available variables:
#   {{domain}}  -- Git server domain ({domain})
#   {{groups}}  -- group path including subgroups, \"/\" separated (org, org/sub)
#   {{project}} -- project name (my-repo)
#   {{branch}}  -- current local Git branch; git command runs only when used in template
#   {{path}}    -- cwd relative to git root; empty when not in a subdirectory
web_url_template = \"https://{{domain}}/{{groups}}/{{project}}/-/tree/{{branch}}/{{path}}\""
        ),
        DomainKind::Bitbucket => format!(
            "\
# Browse URL template -- used by `git-jump browse`.
# Available variables:
#   {{domain}}  -- Git server domain ({domain})
#   {{groups}}  -- group path including subgroups, \"/\" separated (org, org/sub)
#   {{project}} -- project name (my-repo)
#   {{branch}}  -- current local Git branch; git command runs only when used in template
#   {{path}}    -- cwd relative to git root; empty when not in a subdirectory
web_url_template = \"https://{{domain}}/{{groups}}/{{project}}/src/{{branch}}/{{path}}\""
        ),
        DomainKind::Unknown => format!(
            "\
# Browse URL template -- used by `git-jump browse`.
# Available variables:
#   {{domain}}  -- Git server domain ({domain})
#   {{groups}}  -- group path including subgroups, \"/\" separated (org, org/sub)
#   {{project}} -- project name (my-repo)
#   {{branch}}  -- current local Git branch; git command runs only when used in template
#   {{path}}    -- cwd relative to git root; empty when not in a subdirectory
#
# GitHub format:
#   web_url_template = \"https://{{domain}}/{{groups}}/{{project}}/tree/{{branch}}/{{path}}\"
# GitLab format:
#   web_url_template = \"https://{{domain}}/{{groups}}/{{project}}/-/tree/{{branch}}/{{path}}\"
# Bitbucket format:
#   web_url_template = \"https://{{domain}}/{{groups}}/{{project}}/src/{{branch}}/{{path}}\"
# Project home page only:
#   web_url_template = \"https://{{domain}}/{{groups}}/{{project}}\""
        ),
    };

    let logo_example = match kind {
        DomainKind::GitHub => "\"GitHub\"",
        DomainKind::GitLab => "\"GitLab\"",
        DomainKind::Bitbucket => "\"Bitbucket\"",
        DomainKind::Unknown => &format!("\"{domain}\""),
    };

    let alias_short = match kind {
        DomainKind::GitHub => "gh",
        DomainKind::GitLab => "gl",
        DomainKind::Bitbucket => "bb",
        DomainKind::Unknown => "myalias",
    };

    format!(
        "\
# gj (git-jump) -- {domain} domain config
#
# Applies to all projects under {domain}.
# Child configs (.git-jump.toml at group/project level) can override
# or extend these settings.

# Path alias -- create a short name for this domain.
# With alias set, \"{domain}/org/repo\" also becomes \"{alias_short}/org/repo\",
# usable in matching, tab completion, and TUI selector.
# Constraint: must not contain \"/\" or whitespace.
# alias = {alias_example}

{web_url_section}

# ASCII art logo text -- displayed when switching to an environment with a different logo_text.
# Child overrides parent.
# logo_text = {logo_example}

# Git config -- applied automatically on each jump into a project under this domain.
# Merge rule: same key = child overrides parent, different keys = merged.
# Note: dotted keys must be quoted.
# [git_config]
# \"user.name\" = \"Your Name\"
# \"user.email\" = \"you@example.com\"

# Environment variables -- set in shell environment on each jump.
# Merge rule: same key = child overrides parent, different keys = merged.
# [env]
# GOPATH = \"/home/you/go\"

# Hooks -- shell commands to run on each jump.
# Merge rule: append mode, all levels (domain -> group -> project) execute parent-to-child.
# [hooks]
# on_enter = [\"echo 'Entered {domain} project'\"]
"
    )
}

pub(crate) fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_no_tilde() {
        assert_eq!(expand_tilde("/tmp/foo"), PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn expand_tilde_with_tilde() {
        let expanded = expand_tilde("~/code");
        assert!(!expanded.starts_with("~"));
    }

    #[test]
    fn merged_config_child_overrides_parent() {
        let mut merged = MergedConfig::default();

        let parent = LocalConfig {
            web_url_template: Some("https://{domain}/{groups}/{project}".into()),
            git_config: Some(BTreeMap::from([
                ("user.name".into(), "Parent".into()),
                ("user.email".into(), "parent@co.com".into()),
            ])),
            env: Some(BTreeMap::from([("GOPATH".into(), "/go".into())])),
            hooks: Some(Hooks {
                on_enter: vec!["echo parent".into()],
            }),
            ..Default::default()
        };
        merged.apply(&parent);

        let child = LocalConfig {
            web_url_template: Some("https://custom.example.com/{groups}/{project}".into()),
            git_config: Some(BTreeMap::from([("user.name".into(), "Child".into())])),
            env: Some(BTreeMap::from([("RUST_LOG".into(), "debug".into())])),
            hooks: Some(Hooks {
                on_enter: vec!["echo child".into()],
            }),
            ..Default::default()
        };
        merged.apply(&child);

        assert_eq!(
            merged.web_url_template.as_deref(),
            Some("https://custom.example.com/{groups}/{project}")
        );
        assert_eq!(merged.git_config["user.name"], "Child");
        assert_eq!(merged.git_config["user.email"], "parent@co.com");
        assert_eq!(merged.env["GOPATH"], "/go");
        assert_eq!(merged.env["RUST_LOG"], "debug");
        assert_eq!(merged.on_enter_hooks, vec!["echo parent", "echo child"]);
    }

    #[test]
    fn merged_config_logo_text_child_overrides_parent() {
        let mut merged = MergedConfig::default();

        let parent = LocalConfig {
            logo_text: Some("Global".into()),
            ..Default::default()
        };
        merged.apply(&parent);
        assert_eq!(merged.logo_text.as_deref(), Some("Global"));

        let child = LocalConfig {
            logo_text: Some("Domain".into()),
            ..Default::default()
        };
        merged.apply(&child);
        assert_eq!(merged.logo_text.as_deref(), Some("Domain"));
    }

    #[test]
    fn merged_config_logo_text_empty_string_overrides() {
        let mut merged = MergedConfig::default();

        let parent = LocalConfig {
            logo_text: Some("Global".into()),
            ..Default::default()
        };
        merged.apply(&parent);

        let child = LocalConfig {
            logo_text: Some(String::new()),
            ..Default::default()
        };
        merged.apply(&child);
        assert_eq!(merged.logo_text.as_deref(), Some(""));
    }

    #[test]
    fn merged_config_logo_text_none_inherits() {
        let mut merged = MergedConfig::default();

        let parent = LocalConfig {
            logo_text: Some("Global".into()),
            ..Default::default()
        };
        merged.apply(&parent);

        let child = LocalConfig {
            logo_text: None,
            ..Default::default()
        };
        merged.apply(&child);
        assert_eq!(merged.logo_text.as_deref(), Some("Global"));
    }

    #[test]
    fn parse_alias_field() {
        let toml_str = r#"alias = "work""#;
        let config: LocalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.alias.as_deref(), Some("work"));
    }

    #[test]
    fn alias_missing_is_none() {
        let toml_str = r#"[git_config]
"user.name" = "Test"
"#;
        let config: LocalConfig = toml::from_str(toml_str).unwrap();
        assert!(config.alias.is_none());
    }

    #[test]
    fn alias_empty_string_ignored() {
        assert!(!validate_alias(""));
    }

    #[test]
    fn alias_with_slash_ignored() {
        assert!(!validate_alias("work/test"));
    }

    #[test]
    fn alias_with_whitespace_ignored() {
        assert!(!validate_alias("work test"));
        assert!(!validate_alias("work\ttest"));
        assert!(!validate_alias(" work"));
    }

    #[test]
    fn alias_valid_values() {
        assert!(validate_alias("work"));
        assert!(validate_alias("be"));
        assert!(validate_alias("my-alias"));
        assert!(validate_alias("alias_2"));
        assert!(validate_alias("Work"));
    }

    #[test]
    fn load_toml_missing_file_returns_default() {
        let cfg: GlobalConfig =
            load_toml_or_default(Path::new("/nonexistent/path/config.toml")).unwrap();
        assert!(cfg.root.is_none());
    }

    #[test]
    fn load_known_domains_missing_file_returns_empty() {
        // Point XDG to a nonexistent dir so domains file won't be found
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", "/nonexistent/path/that/does/not/exist");
        }
        let result = load_known_domains();
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn register_domain_creates_deduplicates_and_cleans() {
        let tmp = tempfile::TempDir::new().unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        }

        register_domain("github.com").unwrap();
        let domains = load_known_domains().unwrap();
        assert_eq!(domains, vec!["github.com"]);

        register_domain("github.com").unwrap();
        let domains = load_known_domains().unwrap();
        assert_eq!(domains.len(), 1);

        register_domain("gitlab.com").unwrap();
        let domains = load_known_domains().unwrap();
        assert_eq!(domains, vec!["github.com", "gitlab.com"]);

        let domains_path = tmp.path().join("git-jump/domains");
        std::fs::write(&domains_path, "a.com\nb.com\na.com\nb.com\n").unwrap();

        register_domain("c.com").unwrap();
        let domains = load_known_domains().unwrap();
        assert_eq!(domains, vec!["a.com", "b.com", "c.com"]);

        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn classify_domain_known() {
        assert!(matches!(classify_domain("github.com"), DomainKind::GitHub));
        assert!(matches!(classify_domain("gitlab.com"), DomainKind::GitLab));
        assert!(matches!(
            classify_domain("bitbucket.org"),
            DomainKind::Bitbucket
        ));
    }

    #[test]
    fn classify_domain_unknown() {
        assert!(matches!(
            classify_domain("git.example.com"),
            DomainKind::Unknown
        ));
        assert!(matches!(
            classify_domain("gitlab.example.com"),
            DomainKind::Unknown
        ));
    }

    #[test]
    fn format_domain_config_github() {
        let content = format_domain_config("github.com");
        assert!(
            content.contains(
                "web_url_template = \"https://{domain}/{groups}/{project}/tree/{branch}/{path}\""
            ),
            "github.com should have uncommented GitHub-format web_url_template"
        );
        for line in content.lines() {
            if line.contains("web_url_template =") && !line.trim_start().starts_with('#') {
                assert!(
                    line.contains("/tree/"),
                    "active web_url_template should use GitHub format"
                );
            }
        }
    }

    #[test]
    fn format_domain_config_gitlab() {
        let content = format_domain_config("gitlab.com");
        assert!(
            content.contains(
                "web_url_template = \"https://{domain}/{groups}/{project}/-/tree/{branch}/{path}\""
            ),
            "gitlab.com should have uncommented GitLab-format web_url_template"
        );
    }

    #[test]
    fn format_domain_config_bitbucket() {
        let content = format_domain_config("bitbucket.org");
        assert!(
            content.contains(
                "web_url_template = \"https://{domain}/{groups}/{project}/src/{branch}/{path}\""
            ),
            "bitbucket.org should have uncommented Bitbucket-format web_url_template"
        );
    }

    #[test]
    fn format_domain_config_unknown() {
        let content = format_domain_config("git.example.com");
        for line in content.lines() {
            if line.contains("web_url_template") {
                assert!(
                    line.trim_start().starts_with('#'),
                    "unknown domain should have all web_url_template lines commented: {line}"
                );
            }
        }
    }

    #[test]
    fn format_domain_config_unknown_has_references() {
        let content = format_domain_config("git.example.com");
        assert!(
            content.contains("# GitHub format:"),
            "should contain GitHub reference"
        );
        assert!(
            content.contains("# GitLab format:"),
            "should contain GitLab reference"
        );
        assert!(
            content.contains("# Bitbucket format:"),
            "should contain Bitbucket reference"
        );
    }

    #[test]
    fn format_domain_config_contains_all_fields() {
        for domain in &[
            "github.com",
            "gitlab.com",
            "bitbucket.org",
            "git.example.com",
        ] {
            let content = format_domain_config(domain);
            assert!(
                content.contains("alias"),
                "{domain}: should contain alias field"
            );
            assert!(
                content.contains("web_url_template"),
                "{domain}: should contain web_url_template"
            );
            assert!(
                content.contains("git_config"),
                "{domain}: should contain git_config"
            );
            assert!(content.contains("[env]"), "{domain}: should contain env");
            assert!(
                content.contains("[hooks]"),
                "{domain}: should contain hooks"
            );
            assert!(
                content.contains("logo_text"),
                "{domain}: should contain logo_text"
            );
        }
    }

    #[test]
    fn format_domain_config_valid_toml() {
        for domain in &[
            "github.com",
            "gitlab.com",
            "bitbucket.org",
            "git.example.com",
        ] {
            let content = format_domain_config(domain);
            let parsed: std::result::Result<LocalConfig, _> = toml::from_str(&content);
            assert!(
                parsed.is_ok(),
                "{domain}: generated config should be valid TOML, error: {:?}",
                parsed.err()
            );

            let config = parsed.unwrap();
            match *domain {
                "github.com" | "gitlab.com" | "bitbucket.org" => {
                    assert!(
                        config.web_url_template.is_some(),
                        "{domain}: known domain should have Some(web_url_template)"
                    );
                }
                _ => {
                    assert!(
                        config.web_url_template.is_none(),
                        "{domain}: unknown domain should have None web_url_template"
                    );
                }
            }
        }
    }

    #[test]
    fn format_domain_config_domain_in_comments() {
        let content = format_domain_config("git.custom.io");
        assert!(
            content.contains("git.custom.io"),
            "comments should contain the actual domain name"
        );
        assert!(
            content.contains("# gj (git-jump) -- git.custom.io domain config"),
            "header should contain the actual domain"
        );
    }

    #[test]
    fn format_domain_config_english_comments() {
        for domain in &[
            "github.com",
            "gitlab.com",
            "bitbucket.org",
            "git.example.com",
        ] {
            let content = format_domain_config(domain);
            for (i, ch) in content.chars().enumerate() {
                assert!(
                    ch.is_ascii() || ch == '\u{FEFF}',
                    "{domain}: found non-ASCII character '{ch}' at position {i}"
                );
            }
        }
    }

    #[test]
    fn ensure_domain_config_creates_when_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("github.com")).unwrap();

        let mut dbg = crate::debug::DebugLog::new(false);
        let result = ensure_domain_config(root, "github.com", &mut dbg).unwrap();

        assert!(result.is_some(), "should return Some(path) when created");
        let path = result.unwrap();
        assert!(path.exists(), "config file should exist");
        assert_eq!(path, root.join("github.com/.git-jump.toml"));

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("web_url_template"));
    }

    fn assert_commented_fields_are_none(config: &LocalConfig, domain: &str) {
        assert!(
            config.alias.is_none(),
            "{domain}: alias should be None (commented out)"
        );
        assert!(
            config.git_config.is_none(),
            "{domain}: git_config should be None (commented out)"
        );
        assert!(
            config.env.is_none(),
            "{domain}: env should be None (commented out)"
        );
        assert!(
            config.hooks.is_none(),
            "{domain}: hooks should be None (commented out)"
        );
        assert!(
            config.logo_text.is_none(),
            "{domain}: logo_text should be None (commented out)"
        );
    }

    #[test]
    fn format_domain_config_github_deserialized() {
        let content = format_domain_config("github.com");
        let config: LocalConfig = toml::from_str(&content).unwrap();

        assert_eq!(
            config.web_url_template.as_deref(),
            Some("https://{domain}/{groups}/{project}/tree/{branch}/{path}")
        );
        assert_commented_fields_are_none(&config, "github.com");
    }

    #[test]
    fn format_domain_config_gitlab_deserialized() {
        let content = format_domain_config("gitlab.com");
        let config: LocalConfig = toml::from_str(&content).unwrap();

        assert_eq!(
            config.web_url_template.as_deref(),
            Some("https://{domain}/{groups}/{project}/-/tree/{branch}/{path}")
        );
        assert_commented_fields_are_none(&config, "gitlab.com");
    }

    #[test]
    fn format_domain_config_bitbucket_deserialized() {
        let content = format_domain_config("bitbucket.org");
        let config: LocalConfig = toml::from_str(&content).unwrap();

        assert_eq!(
            config.web_url_template.as_deref(),
            Some("https://{domain}/{groups}/{project}/src/{branch}/{path}")
        );
        assert_commented_fields_are_none(&config, "bitbucket.org");
    }

    #[test]
    fn format_domain_config_unknown_deserialized() {
        let content = format_domain_config("git.example.com");
        let config: LocalConfig = toml::from_str(&content).unwrap();

        assert!(
            config.web_url_template.is_none(),
            "unknown domain: web_url_template should be None"
        );
        assert_commented_fields_are_none(&config, "git.example.com");
    }

    #[test]
    fn ensure_domain_config_skips_when_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let domain_dir = root.join("github.com");
        std::fs::create_dir_all(&domain_dir).unwrap();

        let custom_content = "# custom config\nalias = \"gh\"\n";
        std::fs::write(domain_dir.join(".git-jump.toml"), custom_content).unwrap();

        let mut dbg = crate::debug::DebugLog::new(false);
        let result = ensure_domain_config(root, "github.com", &mut dbg).unwrap();

        assert!(result.is_none(), "should return None when config exists");

        let content = std::fs::read_to_string(domain_dir.join(".git-jump.toml")).unwrap();
        assert_eq!(
            content, custom_content,
            "existing config should not be modified"
        );
    }
}
