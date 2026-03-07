use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config;
use crate::debug;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct Project {
    pub domain: String,
    pub groups: Vec<String>,
    pub name: String,
    pub path: PathBuf,
}

impl Project {
    pub fn display_path(&self) -> String {
        if self.domain.is_empty() {
            abbreviate_home(&self.path)
        } else {
            let mut parts = vec![self.domain.as_str()];
            for g in &self.groups {
                parts.push(g.as_str());
            }
            parts.push(self.name.as_str());
            parts.join("/")
        }
    }

    pub fn is_domain_project(&self) -> bool {
        !self.domain.is_empty()
    }
}

fn abbreviate_home(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

/// Walk up from `start` looking for a `.git` entry (dir or file).
/// Returns the directory containing `.git`, without canonicalizing.
pub fn detect_git_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        if ancestor.join(".git").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

#[derive(Debug)]
pub enum ProjectClass {
    Domain { root: PathBuf, project: Project },
    NonDomain { project: Project },
}

/// Classify a git root as domain or non-domain project.
///
/// Domain requires: root exists, git_root is under root, first path component
/// is a known domain.
pub fn classify_project(
    git_root: &Path,
    root: Option<&Path>,
    known_domains: &[String],
    dbg: &mut debug::DebugLog,
) -> ProjectClass {
    if let Some(root) = root {
        if let Some(class) = try_classify_domain(git_root, root, known_domains, dbg) {
            return class;
        }
    }

    let name = git_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    dbg.log("project class: non-domain");
    dbg.log_indent(&format!("path: {}", debug::abbreviate_path(git_root)));

    ProjectClass::NonDomain {
        project: Project {
            domain: String::new(),
            groups: Vec::new(),
            name,
            path: git_root.to_path_buf(),
        },
    }
}

fn try_classify_domain(
    git_root: &Path,
    root: &Path,
    known_domains: &[String],
    dbg: &mut debug::DebugLog,
) -> Option<ProjectClass> {
    let relative = git_root.strip_prefix(root).ok()?;
    let components: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if components.len() < 2 {
        return None;
    }

    let domain = components[0];
    if !known_domains.iter().any(|d| d == domain) {
        return None;
    }

    let name = components.last().unwrap().to_string();
    let groups: Vec<String> = components[1..components.len() - 1]
        .iter()
        .map(|s| s.to_string())
        .collect();

    dbg.log("project class: domain");
    dbg.log_indent(&format!("domain: {domain}"));
    dbg.log_indent(&format!("path: {}", debug::abbreviate_path(git_root)));

    Some(ProjectClass::Domain {
        root: root.to_path_buf(),
        project: Project {
            domain: domain.to_string(),
            groups,
            name,
            path: git_root.to_path_buf(),
        },
    })
}

/// Discover Git projects under `root`, scanning only the given `domains`.
///
/// Each domain corresponds to a top-level subdirectory under `root`.
/// A directory is considered a project if it contains a `.git` entry (directory or file).
pub fn discover(root: &Path, domains: &[String]) -> Vec<Project> {
    let mut projects = Vec::new();

    for domain in domains {
        let path = root.join(domain);
        if !path.is_dir() {
            continue;
        }
        walk_for_projects(&path, domain, &[], &mut projects, 0);
    }

    projects.sort_by_key(|p| p.display_path());
    projects
}

// -- Alias support --

#[derive(Debug, Clone)]
pub struct AliasEntry {
    pub dir_path: PathBuf,
    pub source_path: String,
    pub alias: String,
}

#[derive(Debug, Default)]
pub struct AliasRegistry {
    entries: BTreeMap<PathBuf, AliasEntry>,
}

impl AliasRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, entry: AliasEntry) {
        self.entries.insert(entry.dir_path.clone(), entry);
    }

    pub fn find_nearest_alias(&self, project_path: &Path) -> Option<&AliasEntry> {
        let mut current = project_path;
        loop {
            if let Some(entry) = self.entries.get(current) {
                return Some(entry);
            }
            match current.parent() {
                Some(parent) if parent != current => current = parent,
                _ => break,
            }
        }
        None
    }

    pub fn entries(&self) -> impl Iterator<Item = &AliasEntry> {
        self.entries.values()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub fn load_domain_aliases(
    root: &Path,
    domains: &[String],
    dbg: &mut debug::DebugLog,
) -> Result<AliasRegistry> {
    let mut registry = AliasRegistry::new();

    for domain in domains {
        let domain_dir = root.join(domain);
        if !domain_dir.is_dir() {
            continue;
        }
        check_alias_at(&domain_dir, domain, &mut registry, dbg)?;
        walk_for_aliases(&domain_dir, domain, &mut registry, 0, dbg)?;
    }

    Ok(registry)
}

pub fn load_non_domain_aliases(
    git_root: &Path,
    registry: &mut AliasRegistry,
    dbg: &mut debug::DebugLog,
) -> Result<()> {
    let mut dirs: Vec<&Path> = git_root.ancestors().collect();
    dirs.reverse();

    for dir in dirs {
        let toml_path = dir.join(".git-jump.toml");
        let content = match std::fs::read_to_string(&toml_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => continue,
            Err(_) => continue,
        };
        let local: config::LocalConfig = toml::from_str(&content)?;
        if let Some(alias_val) = &local.alias {
            if !config::validate_alias(alias_val) {
                dbg.log(&format!(
                    "alias: invalid value {:?} at {}, ignored",
                    alias_val,
                    debug::abbreviate_path(dir),
                ));
                continue;
            }
            let source_path = abbreviate_home(dir);
            dbg.log(&format!(
                "alias: {}/ -> {:?}  (non-domain, pinned)",
                source_path, alias_val
            ));
            registry.add(AliasEntry {
                dir_path: dir.to_path_buf(),
                source_path,
                alias: alias_val.clone(),
            });
        }
    }

    Ok(())
}

fn check_alias_at(
    dir: &Path,
    relative_path: &str,
    registry: &mut AliasRegistry,
    dbg: &mut debug::DebugLog,
) -> Result<()> {
    let local = config::load_local_config(dir)?;
    if let Some(alias_val) = &local.alias {
        if !config::validate_alias(alias_val) {
            dbg.log(&format!(
                "alias: invalid value {:?} at {}, ignored",
                alias_val, relative_path,
            ));
            return Ok(());
        }
        registry.add(AliasEntry {
            dir_path: dir.to_path_buf(),
            source_path: relative_path.to_string(),
            alias: alias_val.clone(),
        });
    }
    Ok(())
}

fn walk_for_aliases(
    dir: &Path,
    parent_relative: &str,
    registry: &mut AliasRegistry,
    depth: usize,
    dbg: &mut debug::DebugLog,
) -> Result<()> {
    if depth >= MAX_DEPTH {
        return Ok(());
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match entry.file_name().to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };

        if name.starts_with('.') {
            continue;
        }

        let relative = format!("{parent_relative}/{name}");
        if path.join(".git").exists() {
            check_alias_at(&path, &relative, registry, dbg)?;
        } else {
            check_alias_at(&path, &relative, registry, dbg)?;
            walk_for_aliases(&path, &relative, registry, depth + 1, dbg)?;
        }
    }

    Ok(())
}

const MAX_DEPTH: usize = 8;

fn walk_for_projects(
    dir: &Path,
    domain: &str,
    groups: &[String],
    projects: &mut Vec<Project>,
    depth: usize,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match entry.file_name().to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };

        // Skip hidden directories
        if name.starts_with('.') {
            continue;
        }

        if path.join(".git").exists() {
            // Found a git project -- do NOT recurse further.
            projects.push(Project {
                domain: domain.to_string(),
                groups: groups.to_vec(),
                name,
                path,
            });
        } else if depth < MAX_DEPTH {
            // Potential group/subgroup directory -- recurse.
            let mut child_groups = groups.to_vec();
            child_groups.push(name);
            walk_for_projects(&path, domain, &child_groups, projects, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_project_tree() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // domain1/group1/project-a/.git
        let p = root.join("domain1/group1/project-a/.git");
        fs::create_dir_all(&p).unwrap();

        // domain1/group1/project-b/.git
        let p = root.join("domain1/group1/project-b/.git");
        fs::create_dir_all(&p).unwrap();

        // domain1/group2/sub1/project-c/.git
        let p = root.join("domain1/group2/sub1/project-c/.git");
        fs::create_dir_all(&p).unwrap();

        // domain2/team/project-d/.git
        let p = root.join("domain2/team/project-d/.git");
        fs::create_dir_all(&p).unwrap();

        tmp
    }

    #[test]
    fn discover_finds_all_projects() {
        let tmp = setup_project_tree();
        let projects = discover(tmp.path(), &["domain1".into(), "domain2".into()]);
        assert_eq!(projects.len(), 4);
    }

    #[test]
    fn discover_correct_structure() {
        let tmp = setup_project_tree();
        let projects = discover(tmp.path(), &["domain1".into(), "domain2".into()]);

        let pa = projects.iter().find(|p| p.name == "project-a").unwrap();
        assert_eq!(pa.domain, "domain1");
        assert_eq!(pa.groups, vec!["group1"]);

        let pc = projects.iter().find(|p| p.name == "project-c").unwrap();
        assert_eq!(pc.domain, "domain1");
        assert_eq!(pc.groups, vec!["group2", "sub1"]);
    }

    #[test]
    fn display_path_format() {
        let p = Project {
            domain: "github.com".into(),
            groups: vec!["org".into(), "sub".into()],
            name: "repo".into(),
            path: PathBuf::from("/tmp/github.com/org/sub/repo"),
        };
        assert_eq!(p.display_path(), "github.com/org/sub/repo");
    }

    #[test]
    fn depth_limit_stops_recursion() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create a domain with no .git anywhere, depth > MAX_DEPTH
        // domain/l1/l2/l3/l4/l5/l6/l7/l8/l9 (9 levels of non-git dirs)
        let deep = root.join("domain/l1/l2/l3/l4/l5/l6/l7/l8/l9");
        fs::create_dir_all(&deep).unwrap();

        let projects = discover(root, &["domain".into()]);
        // No .git anywhere -- should return empty, not hang
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn git_file_detected_as_project() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create a project where .git is a file (submodule/worktree pattern)
        let project_dir = root.join("domain/group/my-submodule");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join(".git"), "gitdir: /fake/path\n").unwrap();

        let projects = discover(root, &["domain".into()]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-submodule");
    }

    #[test]
    fn project_at_max_depth_found() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create a project at depth 8 from domain (domain + 7 group levels + project)
        // domain/g1/g2/g3/g4/g5/g6/g7/deep-project/.git
        let git_dir = root.join("domain/g1/g2/g3/g4/g5/g6/g7/deep-project/.git");
        fs::create_dir_all(&git_dir).unwrap();

        let projects = discover(root, &["domain".into()]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "deep-project");
    }

    #[test]
    fn load_aliases_basic() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("domain1/group1/project-a/.git")).unwrap();
        fs::write(root.join("domain1/.git-jump.toml"), "alias = \"work\"\n").unwrap();

        let mut dbg = crate::debug::DebugLog::new(false);
        let registry = super::load_domain_aliases(root, &["domain1".into()], &mut dbg).unwrap();

        assert!(!registry.is_empty());
        let entry = registry.entries.get(&root.join("domain1")).unwrap();
        assert_eq!(entry.alias, "work");
        assert_eq!(entry.source_path, "domain1");
    }

    #[test]
    fn load_aliases_multi_level() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("domain1/backend/project-a/.git")).unwrap();
        fs::write(root.join("domain1/.git-jump.toml"), "alias = \"work\"\n").unwrap();
        fs::write(
            root.join("domain1/backend/.git-jump.toml"),
            "alias = \"be\"\n",
        )
        .unwrap();

        let mut dbg = crate::debug::DebugLog::new(false);
        let registry = super::load_domain_aliases(root, &["domain1".into()], &mut dbg).unwrap();

        let project_path = root.join("domain1/backend/project-a");
        let nearest = registry.find_nearest_alias(&project_path).unwrap();
        assert_eq!(nearest.alias, "be");
    }

    #[test]
    fn load_aliases_same_value_across_domains() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("domain1/group/project-a/.git")).unwrap();
        fs::create_dir_all(root.join("domain2/group/project-b/.git")).unwrap();
        fs::write(root.join("domain1/.git-jump.toml"), "alias = \"work\"\n").unwrap();
        fs::write(root.join("domain2/.git-jump.toml"), "alias = \"work\"\n").unwrap();

        let mut dbg = crate::debug::DebugLog::new(false);
        let registry =
            super::load_domain_aliases(root, &["domain1".into(), "domain2".into()], &mut dbg)
                .unwrap();

        assert_eq!(registry.entries.len(), 2);
        assert_eq!(
            registry.entries.get(&root.join("domain1")).unwrap().alias,
            "work"
        );
        assert_eq!(
            registry.entries.get(&root.join("domain2")).unwrap().alias,
            "work"
        );
    }

    #[test]
    fn load_aliases_toml_error_aborts() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("domain1/group/project-a/.git")).unwrap();
        fs::write(root.join("domain1/.git-jump.toml"), "alias = \"work\n").unwrap();

        let mut dbg = crate::debug::DebugLog::new(false);
        let result = super::load_domain_aliases(root, &["domain1".into()], &mut dbg);
        assert!(result.is_err());
    }

    #[test]
    fn discover_skips_unknown_domains() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create projects under two domains
        fs::create_dir_all(root.join("known.com/group/project-a/.git")).unwrap();
        fs::create_dir_all(root.join("unknown/stuff/nested/deep")).unwrap();

        // Only pass "known.com" as a known domain
        let projects = discover(root, &["known.com".into()]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].domain, "known.com");
    }
}
