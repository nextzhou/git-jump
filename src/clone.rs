use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::config::{self, GlobalConfig};
use crate::debug::{self, DebugLog};
use crate::error::{Error, Result};

pub fn run(repo: &str, global: &GlobalConfig, dbg: &mut DebugLog) -> Result<String> {
    let root = config::resolve_root(global)?;

    let known_domains = config::load_known_domains()?;
    debug::log_env_config(dbg, global, &known_domains)?;

    let parsed = parse_repo(repo)?;

    if dbg.is_enabled() {
        dbg.log(&format!(
            "parsed repo: domain={} groups=[{}] project={}",
            parsed.domain,
            parsed.groups.join(", "),
            parsed.project,
        ));
    }

    let target_dir = build_target_dir(&root, &parsed);

    if dbg.is_enabled() {
        dbg.log(&format!("target: {}", debug::abbreviate_path(&target_dir)));
        dbg.log(&format!("target exists: {}", target_dir.exists()));
    }

    if target_dir.exists() {
        if dbg.is_enabled() {
            dbg.log("target directory exists, skipping clone");
        }
    } else {
        if let Some(parent) = target_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Io {
                source: e,
                path: Some(parent.to_path_buf()),
            })?;
        }

        if dbg.is_enabled() {
            dbg.log(&format!("clone url: {repo}"));
        }

        let t_clone = Instant::now();
        let status = Command::new("git")
            .args(["clone", repo, &target_dir.to_string_lossy()])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()?;
        let clone_secs = t_clone.elapsed().as_secs_f64();

        if dbg.is_enabled() {
            let exit_code = status.code().unwrap_or(-1);
            dbg.log(&format!("git clone: exit {exit_code} ({clone_secs:.1}s)"));
        }

        if !status.success() {
            return Err(Error::Config(format!(
                "git clone failed with exit code {}",
                status
            )));
        }
    }

    // -- unified post-processing --
    config::register_domain(&parsed.domain)?;

    match config::ensure_domain_config(&root, &parsed.domain, dbg) {
        Ok(Some(config_path)) => {
            let display = debug::abbreviate_path(&config_path);
            debug::hint(&format!("created domain config: {display}"));
            debug::hint("review and customize git identity, browse URL, and other settings");
        }
        Ok(None) => {
            // config already exists, nothing to do
        }
        Err(e) => {
            debug::warning(&format!("failed to create domain config: {e}"));
        }
    }

    Ok(target_dir.to_string_lossy().into_owned())
}

#[derive(Debug)]
struct ParsedRepo {
    domain: String,
    groups: Vec<String>,
    project: String,
}

fn parse_repo(repo: &str) -> Result<ParsedRepo> {
    if let Some(rest) = repo
        .strip_prefix("https://")
        .or_else(|| repo.strip_prefix("http://"))
    {
        return parse_url_path(rest);
    }

    if let Some(rest) = repo.strip_prefix("git@") {
        if let Some((domain, path)) = rest.split_once(':') {
            let path = path.strip_suffix(".git").unwrap_or(path);
            let segments = split_path_segments(path)?;
            return Ok(ParsedRepo {
                domain: domain.to_string(),
                groups: segments[..segments.len() - 1].to_vec(),
                project: segments[segments.len() - 1].clone(),
            });
        }
    }

    Err(Error::Config(format!(
        "invalid repo URL: expected https:// or git@ URL, got '{repo}'\n  hint: use full URL, e.g. gjclone https://github.com/group/project"
    )))
}

fn parse_url_path(url_without_scheme: &str) -> Result<ParsedRepo> {
    let (domain, path) = url_without_scheme.split_once('/').ok_or_else(|| {
        Error::Config(format!(
            "invalid repo URL: cannot parse '{url_without_scheme}'"
        ))
    })?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments = split_path_segments(path)?;

    Ok(ParsedRepo {
        domain: domain.to_string(),
        groups: segments[..segments.len() - 1].to_vec(),
        project: segments[segments.len() - 1].clone(),
    })
}

fn split_path_segments(path: &str) -> Result<Vec<String>> {
    let segments: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    if segments.len() < 2 {
        return Err(Error::Config(format!(
            "repo path needs at least group/project: '{path}'"
        )));
    }
    Ok(segments)
}

fn build_target_dir(root: &std::path::Path, parsed: &ParsedRepo) -> PathBuf {
    let mut path = root.join(&parsed.domain);
    for group in &parsed.groups {
        path = path.join(group);
    }
    path.join(&parsed.project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_https_url() {
        let parsed = parse_repo("https://github.com/org/repo.git").unwrap();
        assert_eq!(parsed.domain, "github.com");
        assert_eq!(parsed.groups, vec!["org"]);
        assert_eq!(parsed.project, "repo");
    }

    #[test]
    fn parse_https_url_no_dot_git() {
        let parsed = parse_repo("https://github.com/org/repo").unwrap();
        assert_eq!(parsed.project, "repo");
    }

    #[test]
    fn parse_ssh_url() {
        let parsed = parse_repo("git@github.com:org/repo.git").unwrap();
        assert_eq!(parsed.domain, "github.com");
        assert_eq!(parsed.groups, vec!["org"]);
        assert_eq!(parsed.project, "repo");
    }

    #[test]
    fn parse_shorthand_rejected() {
        let err = parse_repo("backend/api-gateway").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid repo URL"),
            "should reject shorthand, got: {msg}"
        );
        assert!(msg.contains("hint:"), "should include hint, got: {msg}");
    }

    #[test]
    fn parse_single_segment_rejected() {
        let err = parse_repo("onlyone").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid repo URL"),
            "should reject single segment, got: {msg}"
        );
    }

    #[test]
    fn target_dir_structure() {
        let root = PathBuf::from("/code");
        let parsed = ParsedRepo {
            domain: "github.com".into(),
            groups: vec!["org".into(), "sub".into()],
            project: "repo".into(),
        };
        assert_eq!(
            build_target_dir(&root, &parsed),
            PathBuf::from("/code/github.com/org/sub/repo")
        );
    }
}
