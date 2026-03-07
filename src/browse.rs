use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

use crate::config::{self, GlobalConfig, MergedConfig};
use crate::debug::{self, DebugLog};
use crate::error::{Error, Result};
use crate::jump;
use crate::project::{self, Project};
use crate::resolve;

struct BrowseContext {
    /// cwd relative to git root.
    /// Empty string if pattern-based, at git root, or not detectable.
    path: String,
}

pub fn run(pattern: &[String], global: &GlobalConfig, dbg: &mut DebugLog) -> Result<()> {
    let root = config::resolve_root(global)?;

    let known_domains = config::load_known_domains()?;

    let (project, ctx) = if pattern.is_empty() {
        let cwd = std::env::current_dir().ok();
        match cwd
            .as_deref()
            .and_then(|cwd| detect_current_project(&root, cwd, &known_domains))
        {
            Some(p) => {
                let path = cwd
                    .as_deref()
                    .and_then(|cwd| cwd.strip_prefix(&p.path).ok())
                    .map(|rel| rel.to_string_lossy().to_string())
                    .unwrap_or_default();
                if p.is_domain_project() {
                    dbg.log(&format!("current project: {}", p.display_path()));
                } else {
                    dbg.log(&format!(
                        "current project (non-domain): {}",
                        p.display_path()
                    ));
                }
                dbg.log_indent(&format!(
                    "path: {}",
                    if path.is_empty() { "(empty)" } else { &path }
                ));
                (p, BrowseContext { path })
            }
            None => {
                let resolved = resolve::resolve_project(pattern, global, dbg)?;
                (
                    resolved.project,
                    BrowseContext {
                        path: String::new(),
                    },
                )
            }
        }
    } else {
        let resolved = resolve::resolve_project(pattern, global, dbg)?;
        (
            resolved.project,
            BrowseContext {
                path: String::new(),
            },
        )
    };

    let merged = if project.is_domain_project() {
        debug::log_config_chain(dbg, &root, &project.path);
        config::collect_merged_config(&root, &project.path)?
    } else {
        config::collect_merged_config_non_domain(&project.path)
    };

    let url = construct_url(&project, &merged, &ctx, dbg)?;

    println!("{url}");

    if std::io::stdout().is_terminal() {
        open_browser(&url, global.browser.as_deref(), dbg);
    } else {
        dbg.log("browser: skipped (stdout is not a tty)");
    }

    Ok(())
}

fn detect_current_project(root: &Path, cwd: &Path, known_domains: &[String]) -> Option<Project> {
    if let Some(p) = detect_domain_project(root, cwd, known_domains) {
        return Some(p);
    }

    let git_root = project::detect_git_root(cwd)?;
    let name = git_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Some(Project {
        domain: String::new(),
        groups: Vec::new(),
        name,
        path: git_root,
    })
}

fn detect_domain_project(root: &Path, cwd: &Path, known_domains: &[String]) -> Option<Project> {
    let canonical_root = root.canonicalize().ok()?;

    let mut dir = cwd;
    loop {
        if dir.join(".git").exists() && dir.starts_with(&canonical_root) {
            break;
        }
        dir = dir.parent()?;
        if !dir.starts_with(&canonical_root) {
            return None;
        }
    }

    let relative = dir.strip_prefix(&canonical_root).ok()?;
    let components: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if components.len() < 2 {
        return None;
    }

    let domain = components[0].to_string();
    if !known_domains.iter().any(|d| d == &domain) {
        return None;
    }
    let name = components.last().unwrap().to_string();
    let groups: Vec<String> = components[1..components.len() - 1]
        .iter()
        .map(|s| s.to_string())
        .collect();

    Some(Project {
        domain,
        groups,
        name,
        path: root.join(relative),
    })
}

fn construct_url(
    project: &Project,
    merged: &MergedConfig,
    ctx: &BrowseContext,
    dbg: &mut DebugLog,
) -> Result<String> {
    let template = if let Some(tmpl) = &merged.web_url_template {
        dbg.log("url source: web_url_template");
        dbg.log_indent(&format!("template: {tmpl}"));
        tmpl.as_str()
    } else if project.is_domain_project() {
        dbg.log("url source: default inference");
        "https://{domain}/{groups}/{project}"
    } else {
        return Err(Error::NoWebUrlTemplate);
    };

    let branch = if template.contains("{branch}") {
        let b = detect_branch(&project.path)?;
        if b == "HEAD" {
            eprintln!("gj: warning: HEAD is detached, browse URL may not work as expected");
        }
        dbg.log_indent(&format!("branch: {b}"));
        Some(b)
    } else {
        None
    };

    if template.contains("{path}") || branch.is_some() {
        let path_display = if ctx.path.is_empty() {
            "(empty)"
        } else {
            &ctx.path
        };
        dbg.log_indent(&format!("path: {path_display}"));
    }

    let url = render_template(template, project, branch.as_deref(), &ctx.path);
    dbg.log_indent(&format!("url: {url}"));
    Ok(url)
}

fn detect_branch(project_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args([
            "-C",
            &project_path.to_string_lossy(),
            "rev-parse",
            "--abbrev-ref",
            "HEAD",
        ])
        .output()
        .map_err(|_| Error::BranchDetectFailed {
            project: project_path.display().to_string(),
        })?;

    if !output.status.success() {
        return Err(Error::BranchDetectFailed {
            project: project_path.display().to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn render_template(template: &str, project: &Project, branch: Option<&str>, path: &str) -> String {
    let groups_joined = project.groups.join("/");
    let mut raw = template
        .replace("{domain}", &project.domain)
        .replace("{groups}", &groups_joined)
        .replace("{project}", &project.name);
    if let Some(b) = branch {
        raw = raw.replace("{branch}", b);
    }
    raw = raw.replace("{path}", path);
    normalize_slashes(&raw)
}

fn normalize_slashes(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        return format!("https://{}", collapse_slashes(rest));
    }
    if let Some(rest) = url.strip_prefix("http://") {
        return format!("http://{}", collapse_slashes(rest));
    }
    collapse_slashes(url)
}

fn collapse_slashes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_slash = false;
    for ch in s.chars() {
        if ch == '/' {
            if !prev_slash {
                result.push('/');
            }
            prev_slash = true;
        } else {
            result.push(ch);
            prev_slash = false;
        }
    }
    result
}

fn open_browser(url: &str, browser_config: Option<&str>, dbg: &mut DebugLog) {
    match browser_config {
        Some(template) => {
            let escaped_url = jump::shell_escape(url);
            let cmd = template.replace("{url}", &escaped_url);
            dbg.log(&format!("browser: custom command: {cmd}"));
            match Command::new("sh").args(["-c", &cmd]).spawn() {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("gj: failed to open browser: {e}");
                    eprintln!("  hint: check 'browser' in ~/.config/git-jump/config.toml");
                }
            }
        }
        None => {
            dbg.log("browser: webbrowser crate (auto-detect)");
            if let Err(e) = webbrowser::open(url) {
                eprintln!("gj: failed to open browser: {e}");
                eprintln!("  hint: set 'browser' in ~/.config/git-jump/config.toml");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_project(domain: &str, groups: &[&str], name: &str) -> Project {
        let mut path_parts = vec!["/code", domain];
        path_parts.extend_from_slice(groups);
        path_parts.push(name);
        Project {
            domain: domain.into(),
            groups: groups.iter().map(|s| s.to_string()).collect(),
            name: name.into(),
            path: PathBuf::from(path_parts.join("/")),
        }
    }

    fn empty_ctx() -> BrowseContext {
        BrowseContext {
            path: String::new(),
        }
    }

    fn ctx_with_path(path: &str) -> BrowseContext {
        BrowseContext {
            path: path.to_string(),
        }
    }

    #[test]
    fn render_template_basic() {
        let p = make_project("github.com", &["org"], "my-repo");
        let url = render_template("https://{domain}/{groups}/{project}", &p, None, "");
        assert_eq!(url, "https://github.com/org/my-repo");
    }

    #[test]
    fn render_template_custom_port() {
        let p = make_project("git.example.com", &["backend"], "api");
        let url = render_template("https://{domain}:8443/{groups}/{project}", &p, None, "");
        assert_eq!(url, "https://git.example.com:8443/backend/api");
    }

    #[test]
    fn render_template_subgroups() {
        let p = make_project("github.com", &["org", "sub"], "deep-project");
        let url = render_template("https://{domain}/{groups}/{project}", &p, None, "");
        assert_eq!(url, "https://github.com/org/sub/deep-project");
    }

    #[test]
    fn render_template_empty_groups() {
        let p = make_project("github.com", &[], "solo-project");
        let url = render_template("https://{domain}/{groups}/{project}", &p, None, "");
        assert_eq!(url, "https://github.com/solo-project");
    }

    #[test]
    fn render_template_with_branch_and_path() {
        let p = make_project("github.com", &["org"], "my-repo");
        let url = render_template(
            "https://{domain}/{groups}/{project}/-/tree/{branch}/{path}",
            &p,
            Some("master"),
            "charts/apollo",
        );
        assert_eq!(
            url,
            "https://github.com/org/my-repo/-/tree/master/charts/apollo"
        );
    }

    #[test]
    fn render_template_branch_with_slash() {
        let p = make_project("github.com", &["org"], "my-repo");
        let url = render_template(
            "https://{domain}/{groups}/{project}/tree/{branch}/{path}",
            &p,
            Some("feature/login"),
            "src",
        );
        assert_eq!(url, "https://github.com/org/my-repo/tree/feature/login/src");
    }

    #[test]
    fn render_template_empty_path() {
        let p = make_project("github.com", &["org"], "my-repo");
        let url = render_template(
            "https://{domain}/{groups}/{project}/-/tree/{branch}/{path}",
            &p,
            Some("master"),
            "",
        );
        assert_eq!(url, "https://github.com/org/my-repo/-/tree/master/");
    }

    #[test]
    fn render_template_branch_none_not_replaced() {
        let p = make_project("github.com", &["org"], "my-repo");
        let url = render_template("https://{domain}/{groups}/{project}", &p, None, "");
        assert_eq!(url, "https://github.com/org/my-repo");
    }

    #[test]
    fn render_template_path_only_no_branch() {
        let p = make_project("github.com", &["org"], "my-repo");
        let url = render_template(
            "https://{domain}/{groups}/{project}/browse/{path}",
            &p,
            None,
            "charts/apollo",
        );
        assert_eq!(url, "https://github.com/org/my-repo/browse/charts/apollo");
    }

    #[test]
    fn normalize_slashes_preserves_protocol() {
        assert_eq!(
            normalize_slashes("https://github.com//org//repo"),
            "https://github.com/org/repo"
        );
    }

    #[test]
    fn normalize_slashes_http() {
        assert_eq!(
            normalize_slashes("http://example.com//path"),
            "http://example.com/path"
        );
    }

    #[test]
    fn normalize_slashes_no_protocol() {
        assert_eq!(normalize_slashes("foo//bar///baz"), "foo/bar/baz");
    }

    #[test]
    fn normalize_slashes_no_doubles() {
        assert_eq!(
            normalize_slashes("https://github.com/org/repo"),
            "https://github.com/org/repo"
        );
    }

    #[test]
    fn construct_url_template_over_default() {
        let p = make_project("git.example.com", &["backend"], "api");
        let merged = MergedConfig {
            web_url_template: Some("https://{domain}:8443/{groups}/{project}".into()),
            ..Default::default()
        };
        let mut dbg = crate::debug::DebugLog::new(false);
        let url = construct_url(&p, &merged, &empty_ctx(), &mut dbg).unwrap();
        assert_eq!(url, "https://git.example.com:8443/backend/api");
    }

    #[test]
    fn construct_url_default_inference() {
        let p = make_project("github.com", &["org"], "my-repo");
        let merged = MergedConfig::default();
        let mut dbg = crate::debug::DebugLog::new(false);
        let url = construct_url(&p, &merged, &empty_ctx(), &mut dbg).unwrap();
        assert_eq!(url, "https://github.com/org/my-repo");
    }

    #[test]
    fn construct_url_non_domain_with_static_template() {
        let p = Project {
            domain: String::new(),
            groups: vec![],
            name: "my-project".into(),
            path: PathBuf::from("/home/user/my-project"),
        };
        let merged = MergedConfig {
            web_url_template: Some("https://github.com/user/my-project".into()),
            ..Default::default()
        };
        let mut dbg = crate::debug::DebugLog::new(false);
        let url = construct_url(&p, &merged, &empty_ctx(), &mut dbg).unwrap();
        assert_eq!(url, "https://github.com/user/my-project");
    }

    #[test]
    fn construct_url_non_domain_without_template_errors() {
        let p = Project {
            domain: String::new(),
            groups: vec![],
            name: "my-project".into(),
            path: PathBuf::from("/home/user/my-project"),
        };
        let merged = MergedConfig::default();
        let mut dbg = crate::debug::DebugLog::new(false);
        assert!(construct_url(&p, &merged, &empty_ctx(), &mut dbg).is_err());
    }

    #[test]
    fn construct_url_static_template_no_placeholders() {
        let p = make_project("git.example.com", &["team"], "special");
        let merged = MergedConfig {
            web_url_template: Some("https://custom.example.com/special".into()),
            ..Default::default()
        };
        let mut dbg = crate::debug::DebugLog::new(false);
        let url = construct_url(&p, &merged, &ctx_with_path("src/lib"), &mut dbg).unwrap();
        assert_eq!(url, "https://custom.example.com/special");
    }

    #[test]
    fn construct_url_path_only_template() {
        let p = make_project("git.example.com", &["devops"], "helm-charts");
        let merged = MergedConfig {
            web_url_template: Some("https://{domain}/{groups}/{project}/browse/{path}".into()),
            ..Default::default()
        };
        let mut dbg = crate::debug::DebugLog::new(false);
        let url = construct_url(&p, &merged, &ctx_with_path("charts/apollo"), &mut dbg).unwrap();
        assert_eq!(
            url,
            "https://git.example.com/devops/helm-charts/browse/charts/apollo"
        );
    }

    #[test]
    fn detect_branch_real_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path().join("test-repo");
        std::fs::create_dir_all(&repo).unwrap();
        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(&repo)
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &repo.to_string_lossy(),
                "config",
                "user.email",
                "test@test.com",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args(["-C", &repo.to_string_lossy(), "config", "user.name", "Test"])
            .output()
            .unwrap();
        // Create an initial commit so HEAD is valid
        Command::new("git")
            .args([
                "-C",
                &repo.to_string_lossy(),
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ])
            .output()
            .unwrap();

        let branch = detect_branch(&repo).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn detect_branch_detached_head() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path().join("test-repo");
        std::fs::create_dir_all(&repo).unwrap();
        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(&repo)
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &repo.to_string_lossy(),
                "config",
                "user.email",
                "test@test.com",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args(["-C", &repo.to_string_lossy(), "config", "user.name", "Test"])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &repo.to_string_lossy(),
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ])
            .output()
            .unwrap();
        Command::new("git")
            .args(["-C", &repo.to_string_lossy(), "checkout", "--detach"])
            .output()
            .unwrap();

        let branch = detect_branch(&repo).unwrap();
        assert_eq!(branch, "HEAD");
    }

    #[test]
    fn detect_branch_invalid_repo_fails() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bad_repo = tmp.path().join("not-a-repo");
        std::fs::create_dir_all(&bad_repo).unwrap();

        assert!(detect_branch(&bad_repo).is_err());
    }

    #[test]
    fn collapse_slashes_basic() {
        assert_eq!(collapse_slashes("a//b///c"), "a/b/c");
    }

    #[test]
    fn collapse_slashes_no_doubles() {
        assert_eq!(collapse_slashes("a/b/c"), "a/b/c");
    }

    #[test]
    fn collapse_slashes_trailing() {
        assert_eq!(collapse_slashes("a/b//"), "a/b/");
    }

    #[test]
    fn detect_current_project_in_project_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let domains = vec!["github.com".to_string()];

        let project_dir = root.join("github.com/org/my-repo");
        std::fs::create_dir_all(project_dir.join(".git")).unwrap();

        let p = detect_current_project(&root, &project_dir, &domains).unwrap();
        assert_eq!(p.domain, "github.com");
        assert_eq!(p.groups, vec!["org"]);
        assert_eq!(p.name, "my-repo");
        assert_eq!(p.path, project_dir);
    }

    #[test]
    fn detect_current_project_in_subdirectory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let domains = vec!["github.com".to_string()];

        let project_dir = root.join("github.com/org/my-repo");
        std::fs::create_dir_all(project_dir.join(".git")).unwrap();
        let sub_dir = project_dir.join("src/lib");
        std::fs::create_dir_all(&sub_dir).unwrap();

        let p = detect_current_project(&root, &sub_dir, &domains).unwrap();
        assert_eq!(p.name, "my-repo");
        assert_eq!(p.path, project_dir);
    }

    #[test]
    fn detect_current_project_outside_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("code");
        std::fs::create_dir_all(&root).unwrap();
        let domains = vec!["github.com".to_string()];

        let outside = tmp.path().join("other");
        std::fs::create_dir_all(&outside).unwrap();

        assert!(detect_current_project(&root, &outside, &domains).is_none());
    }

    #[test]
    fn detect_current_project_no_git() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let domains = vec!["github.com".to_string()];

        let dir = root.join("github.com/org/not-a-project");
        std::fs::create_dir_all(&dir).unwrap();

        assert!(detect_current_project(&root, &dir, &domains).is_none());
    }

    #[test]
    fn detect_current_project_with_subgroups() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let domains = vec!["github.com".to_string()];

        let project_dir = root.join("github.com/org/sub/deep-project");
        std::fs::create_dir_all(project_dir.join(".git")).unwrap();

        let p = detect_current_project(&root, &project_dir, &domains).unwrap();
        assert_eq!(p.domain, "github.com");
        assert_eq!(p.groups, vec!["org", "sub"]);
        assert_eq!(p.name, "deep-project");
    }

    #[test]
    fn detect_current_project_unknown_domain_falls_back_to_non_domain() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let domains = vec!["github.com".to_string()];

        let project_dir = root.join("not-a-domain/org/some-repo");
        std::fs::create_dir_all(project_dir.join(".git")).unwrap();

        let p = detect_current_project(&root, &project_dir, &domains).unwrap();
        assert!(p.domain.is_empty());
        assert_eq!(p.name, "some-repo");
        assert_eq!(p.path, project_dir);
    }
}
