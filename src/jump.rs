use crate::config::{self, GlobalConfig, MergedConfig};
use crate::debug::{self, DebugLog};
use crate::error::{Error, Result};
use crate::project::{self, Project, ProjectClass};
use crate::resolve;

pub fn run(
    pattern: &[String],
    global: Option<&GlobalConfig>,
    dbg: &mut DebugLog,
) -> Result<String> {
    if pattern.len() == 1 && pattern[0] == "." {
        return handle_dot_jump(global, dbg);
    }

    let global = global.ok_or(Error::SetupRequired)?;

    let expanded = expand_dot_tokens(pattern);
    let pattern_ref = if expanded.is_some() {
        expanded.as_deref().unwrap()
    } else {
        pattern
    };

    let resolved = resolve::resolve_project(pattern_ref, global, dbg)?;

    if resolved.project.is_domain_project() {
        debug::log_config_chain(dbg, &resolved.root, &resolved.project.path);
    }

    let mut merged = if resolved.project.is_domain_project() {
        config::collect_merged_config(&resolved.root, &resolved.project.path)?
    } else {
        config::collect_merged_config_non_domain(&resolved.project.path)
    };

    if merged.logo_text.is_none() {
        merged.logo_text = global.logo_text.clone();
    }

    let output = build_shell_commands(&resolved.project, &merged);

    debug::log_shell_commands(dbg, &output);

    Ok(output)
}

fn handle_dot_jump(global: Option<&GlobalConfig>, dbg: &mut DebugLog) -> Result<String> {
    let cwd = std::env::current_dir()?;
    dbg.log(&format!("dot jump: cwd = {}", debug::abbreviate_path(&cwd)));

    let git_root = project::detect_git_root(&cwd).ok_or(Error::NotInGitRepo)?;
    dbg.log(&format!("git root: {}", debug::abbreviate_path(&git_root)));

    let (root, known_domains) = load_domain_context(global);

    let class = project::classify_project(&git_root, root.as_deref(), &known_domains, dbg);

    let (project, mut merged) = match class {
        ProjectClass::Domain { root, project } => {
            debug::log_config_chain(dbg, &root, &project.path);
            let merged = config::collect_merged_config(&root, &project.path)?;
            (project, merged)
        }
        ProjectClass::NonDomain { project } => {
            log_non_domain_config_chain(dbg, &git_root);
            let merged = config::collect_merged_config_non_domain(&git_root);
            (project, merged)
        }
    };

    if merged.logo_text.is_none() {
        if let Some(g) = global {
            merged.logo_text = g.logo_text.clone();
        }
    }

    let output = build_shell_commands(&project, &merged);
    debug::log_shell_commands(dbg, &output);
    Ok(output)
}

fn load_domain_context(global: Option<&GlobalConfig>) -> (Option<std::path::PathBuf>, Vec<String>) {
    let global = match global {
        Some(g) => g,
        None => return (None, Vec::new()),
    };

    let root = config::resolve_root(global).ok();
    let known_domains = config::load_known_domains().unwrap_or_default();

    (root, known_domains)
}

fn expand_dot_tokens(pattern: &[String]) -> Option<Vec<String>> {
    if pattern.len() <= 1 {
        return None;
    }
    if !pattern.iter().any(|t| t == ".") {
        return None;
    }

    let cwd = std::env::current_dir().ok()?;
    let dir_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())?;

    let expanded: Vec<String> = pattern
        .iter()
        .map(|t| {
            if t == "." {
                dir_name.clone()
            } else {
                t.clone()
            }
        })
        .collect();

    Some(expanded)
}

fn log_non_domain_config_chain(dbg: &mut DebugLog, git_root: &std::path::Path) {
    if !dbg.is_enabled() {
        return;
    }

    dbg.log("config chain (non-domain):");
    let mut dirs: Vec<&std::path::Path> = git_root.ancestors().collect();
    dirs.reverse();

    for dir in dirs {
        let toml_path = dir.join(".git-jump.toml");
        let status = if toml_path.exists() {
            "found"
        } else {
            "not found"
        };
        dbg.log_indent(&format!(
            "{} ({status})",
            debug::abbreviate_path(&toml_path)
        ));
    }
}

pub(crate) fn build_shell_commands(project: &Project, merged: &MergedConfig) -> String {
    let mut cmds = Vec::new();

    let logo_text = merged.logo_text.as_deref().unwrap_or("");
    cmds.push(format!(
        "export _GIT_JUMP_LOGO_TEXT={}",
        shell_escape(logo_text)
    ));

    cmds.push(format!(
        "builtin cd -- {}",
        shell_escape(&project.path.to_string_lossy())
    ));

    for (key, value) in &merged.env {
        cmds.push(format!("export {}={}", key, shell_escape(value)));
    }

    for (key, value) in &merged.git_config {
        cmds.push(format!(
            "git config {} {}",
            shell_escape(key),
            shell_escape(value)
        ));
    }

    for hook in &merged.on_enter_hooks {
        cmds.push(hook.clone());
    }

    cmds.join("\n")
}

/// POSIX-safe single-quote escaping: wrap in `'...'` and replace internal `'`
/// with `'\''`.
pub(crate) fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn shell_escape_simple() {
        assert_eq!(shell_escape("hello"), "'hello'");
    }

    #[test]
    fn shell_escape_with_single_quote() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn shell_escape_with_spaces() {
        assert_eq!(shell_escape("path with spaces"), "'path with spaces'");
    }

    #[test]
    fn build_commands_cd_only() {
        let project = Project {
            domain: "github.com".into(),
            groups: vec!["org".into()],
            name: "repo".into(),
            path: "/code/github.com/org/repo".into(),
        };
        let merged = MergedConfig::default();
        let cmds = build_shell_commands(&project, &merged);
        assert!(cmds.starts_with("export _GIT_JUMP_LOGO_TEXT=''\n"));
        assert!(cmds.contains("builtin cd -- '/code/github.com/org/repo'"));
    }

    #[test]
    fn build_commands_with_env_and_hooks() {
        let project = Project {
            domain: "github.com".into(),
            groups: vec![],
            name: "repo".into(),
            path: PathBuf::from("/code/github.com/repo"),
        };
        let merged = MergedConfig {
            git_config: BTreeMap::from([("user.name".into(), "Alice".into())]),
            env: BTreeMap::from([("GOPATH".into(), "/go".into())]),
            on_enter_hooks: vec!["echo welcome".into()],
            ..Default::default()
        };
        let cmds = build_shell_commands(&project, &merged);
        assert!(cmds.starts_with("export _GIT_JUMP_LOGO_TEXT=''\n"));
        assert!(cmds.contains("builtin cd -- '/code/github.com/repo'"));
        assert!(cmds.contains("export GOPATH='/go'"));
        assert!(cmds.contains("git config 'user.name' 'Alice'"));
        assert!(cmds.contains("echo welcome"));
    }

    #[test]
    fn build_commands_always_outputs_logo_text() {
        let project = Project {
            domain: "github.com".into(),
            groups: vec!["org".into()],
            name: "repo".into(),
            path: "/code/github.com/org/repo".into(),
        };
        let merged = MergedConfig::default();
        let cmds = build_shell_commands(&project, &merged);
        assert!(cmds.contains("export _GIT_JUMP_LOGO_TEXT=''"));
        assert!(!cmds.contains("_GIT_JUMP_HAS_CONFIG"));
    }

    #[test]
    fn build_commands_with_logo_text() {
        let project = Project {
            domain: String::new(),
            groups: vec![],
            name: "repo".into(),
            path: "/tmp/repo".into(),
        };
        let merged = MergedConfig {
            logo_text: Some("Company".into()),
            env: BTreeMap::from([("FOO".into(), "bar".into())]),
            ..Default::default()
        };
        let cmds = build_shell_commands(&project, &merged);
        assert!(cmds.starts_with("export _GIT_JUMP_LOGO_TEXT='Company'\n"));
        assert!(cmds.contains("export FOO='bar'"));
    }
}
