use std::path::{Path, PathBuf};

use crate::config;
use crate::error::Result;

/// Collects debug messages for deferred output to stderr.
///
/// All messages are buffered and flushed at once when the command finishes
/// (success or failure). This avoids interfering with interactive TUI and
/// keeps debug output grouped.
pub struct DebugLog {
    enabled: bool,
    messages: Vec<String>,
}

impl DebugLog {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            messages: Vec::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Append a top-level debug line: `DEBUG: {msg}`.
    pub fn log(&mut self, msg: &str) {
        if self.enabled {
            self.messages.push(format!("DEBUG: {msg}"));
        }
    }

    /// Append an indented debug line: `DEBUG:   {msg}` (2-space indent).
    pub fn log_indent(&mut self, msg: &str) {
        if self.enabled {
            self.messages.push(format!("DEBUG:   {msg}"));
        }
    }

    /// Write all buffered messages to stderr.
    pub fn flush(&self) {
        for msg in &self.messages {
            eprintln!("{msg}");
        }
    }
}

/// Replace `$HOME` prefix with `~` for display.
pub fn abbreviate_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

/// Log Environment & Config section (shared by jump, clone, completions).
pub fn log_env_config(
    dbg: &mut DebugLog,
    global: &config::GlobalConfig,
    known_domains: &[String],
) -> Result<()> {
    if !dbg.is_enabled() {
        return Ok(());
    }

    let config_path = config::global_config_path()?;
    dbg.log(&format!("config: {}", abbreviate_path(&config_path)));

    let root_info = if let Ok(env_root) = std::env::var("_GIT_JUMP_ROOT") {
        format!(
            "{} (from $_GIT_JUMP_ROOT)",
            abbreviate_path(&PathBuf::from(&env_root))
        )
    } else if let Some(root) = &global.root {
        let expanded = config::expand_tilde(root);
        format!("{} (from config)", abbreviate_path(&expanded))
    } else {
        "not set".to_string()
    };
    dbg.log_indent(&format!("root: {root_info}"));

    dbg.log(&format!("known domains: {}", known_domains.join(", ")));

    Ok(())
}

/// Log config chain: list each `.git-jump.toml` from root to project.
pub fn log_config_chain(dbg: &mut DebugLog, root: &Path, project_path: &Path) {
    if !dbg.is_enabled() {
        return;
    }

    let relative = match project_path.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => return,
    };

    dbg.log("config chain:");
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        let toml_path = current.join(".git-jump.toml");
        let status = if toml_path.exists() {
            "found"
        } else {
            "not found"
        };
        dbg.log_indent(&format!("{} ({status})", abbreviate_path(&toml_path)));
    }
}

pub fn log_aliases(dbg: &mut DebugLog, registry: &crate::project::AliasRegistry) {
    if !dbg.is_enabled() {
        return;
    }

    if registry.is_empty() {
        dbg.log("aliases: none");
        return;
    }

    let entries: Vec<_> = registry.entries().collect();
    let mut unique_values: Vec<&str> = entries.iter().map(|e| e.alias.as_str()).collect();
    unique_values.sort();
    unique_values.dedup();

    dbg.log("aliases:");
    for entry in &entries {
        dbg.log_indent(&format!("{}/ -> {:?}", entry.source_path, entry.alias));
    }
    dbg.log(&format!(
        "alias values: {} unique ({:?})",
        unique_values.len(),
        unique_values.join("\", \"")
    ));
}

pub fn log_collisions(dbg: &mut DebugLog, candidates: &[crate::resolve::DisplayCandidate]) {
    if !dbg.is_enabled() {
        return;
    }

    let collisions: Vec<_> = candidates
        .iter()
        .filter(|c| c.disambiguation.is_some())
        .collect();

    if collisions.is_empty() {
        dbg.log("display collisions: none");
    } else {
        let mut collision_groups: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for c in &collisions {
            if let Some(suffix) = &c.disambiguation {
                collision_groups
                    .entry(c.display_text.clone())
                    .or_default()
                    .push(suffix.clone());
            }
        }
        dbg.log("display collisions:");
        for (text, sources) in &collision_groups {
            dbg.log_indent(&format!(
                "{:?} -> {} projects ({})",
                text,
                sources.len(),
                sources.join(", ")
            ));
        }
    }
}

/// Output a dim-styled hint message to stderr.
///
/// When stderr is not a TTY or `NO_COLOR` is set, outputs plain text (no ANSI).
pub fn hint(msg: &str) {
    use std::io::IsTerminal;
    let use_color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    if use_color {
        eprintln!("\x1b[2mhint: {msg}\x1b[0m");
    } else {
        eprintln!("hint: {msg}");
    }
}

/// Output a warning message to stderr (same color-degradation rules as `hint`).
pub fn warning(msg: &str) {
    use std::io::IsTerminal;
    let use_color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    if use_color {
        eprintln!("\x1b[2mwarning: {msg}\x1b[0m");
    } else {
        eprintln!("warning: {msg}");
    }
}

pub fn log_shell_commands(dbg: &mut DebugLog, commands: &str) {
    if !dbg.is_enabled() {
        return;
    }

    dbg.log("shell commands:");
    for line in commands.lines() {
        dbg.log_indent(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn disabled_log_collects_nothing() {
        let mut log = DebugLog::new(false);
        log.log("top level");
        log.log_indent("indented");
        assert!(log.messages.is_empty());
    }

    #[test]
    fn enabled_log_collects_messages() {
        let mut log = DebugLog::new(true);
        log.log("top level");
        log.log_indent("indented");
        assert_eq!(log.messages.len(), 2);
        assert_eq!(log.messages[0], "DEBUG: top level");
        assert_eq!(log.messages[1], "DEBUG:   indented");
    }

    #[test]
    fn abbreviate_path_under_home() {
        if let Some(home) = dirs::home_dir() {
            let path = home.join("Documents/code");
            assert_eq!(abbreviate_path(&path), "~/Documents/code");
        }
    }

    #[test]
    fn abbreviate_path_not_under_home() {
        let path = PathBuf::from("/tmp/foo/bar");
        assert_eq!(abbreviate_path(&path), "/tmp/foo/bar");
    }

    #[test]
    fn abbreviate_path_exact_home() {
        if let Some(home) = dirs::home_dir() {
            let result = abbreviate_path(&home);
            assert_eq!(result, "~/");
        }
    }
}
