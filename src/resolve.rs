use std::collections::{BTreeMap, HashMap};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use crate::config::{self, GlobalConfig};
use crate::debug::{self, DebugLog};
use crate::error::{Error, Result};
use crate::filter;
use crate::project::{self, AliasRegistry, Project};
use crate::score;
use crate::select;

pub struct ResolvedProject {
    pub project: Project,
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DisplayCandidate {
    pub project_index: usize,
    pub display_text: String,
    pub is_alias: bool,
    pub alias_source_path: Option<String>,
    pub disambiguation: Option<String>,
}

pub fn resolve_project(
    pattern: &[String],
    global: &GlobalConfig,
    dbg: &mut DebugLog,
) -> Result<ResolvedProject> {
    let root = config::resolve_root(global)?;

    let known_domains = config::load_known_domains()?;

    debug::log_env_config(dbg, global, &known_domains)?;

    let t_discover = Instant::now();
    let all_projects = project::discover(&root, &known_domains);
    let discover_elapsed = t_discover.elapsed();

    if dbg.is_enabled() {
        let mut domain_counts: BTreeMap<&str, usize> = BTreeMap::new();
        for p in &all_projects {
            *domain_counts.entry(&p.domain).or_default() += 1;
        }
        dbg.log(&format!(
            "discovery: {} projects in {} domains ({:.1}ms)",
            all_projects.len(),
            domain_counts.len(),
            discover_elapsed.as_secs_f64() * 1000.0
        ));
        for (d, count) in &domain_counts {
            dbg.log_indent(&format!("{d}: {count} projects"));
        }
    }

    let mut candidates: Vec<Project> = all_projects;

    let mut alias_registry = project::load_domain_aliases(&root, &known_domains, dbg)?;

    let should_pin_current = pattern.is_empty();
    let pinned_non_domain = if should_pin_current {
        pin_current_project(&mut candidates, &root, &known_domains, dbg)
    } else {
        false
    };
    if pinned_non_domain {
        if let Some(p) = candidates.first() {
            project::load_non_domain_aliases(&p.path, &mut alias_registry, dbg)?;
        }
    }

    if candidates.is_empty() {
        return Err(Error::NoMatch {
            pattern: pattern.join(" "),
        });
    }

    debug::log_aliases(dbg, &alias_registry);

    let mut display_candidates = build_display_candidates(&candidates, &alias_registry);
    detect_collisions(&mut display_candidates);

    debug::log_collisions(dbg, &display_candidates);

    let initial_filter = pattern.join(" ");

    let scorer = |project_idx: usize, query: &str| -> (f64, f64) {
        let s = score::score(&candidates[project_idx], query);
        (s.project_score, s.group_score)
    };

    if dbg.is_enabled() {
        let display_texts: Vec<String> = display_candidates
            .iter()
            .map(|c| c.display_text.clone())
            .collect();
        let pre_select_count = if initial_filter.is_empty() {
            candidates.len()
        } else {
            let matches = filter::filter_candidates(&display_texts, &initial_filter);
            let deduped = dedup_filter_matches(&display_candidates, &matches);
            deduped.len()
        };
        let match_word = if pre_select_count == 1 {
            "match"
        } else {
            "matches"
        };
        dbg.log(&format!(
            "candidates: {} -> {} {match_word}",
            candidates.len(),
            pre_select_count
        ));

        let mode = if !initial_filter.is_empty() && pre_select_count == 1 {
            "fast path"
        } else if !std::io::stderr().is_terminal() {
            "non-tty"
        } else {
            "interactive"
        };
        dbg.log(&format!("selection: {mode}"));
    }

    let result = select::select(&display_candidates, &initial_filter, scorer)?;
    let project = candidates[result.index].clone();

    if dbg.is_enabled() {
        dbg.log(&format!("selected: {}", project.display_path()));
        log_scoring(dbg, &candidates, &display_candidates, &result);
    }

    Ok(ResolvedProject { project, root })
}

/// Returns true if the pinned project is non-domain.
fn pin_current_project(
    candidates: &mut Vec<Project>,
    root: &std::path::Path,
    known_domains: &[String],
    dbg: &mut DebugLog,
) -> bool {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let git_root = match project::detect_git_root(&cwd) {
        Some(r) => r,
        None => return false,
    };

    if let Some(pos) = candidates.iter().position(|p| p.path == git_root) {
        if pos != 0 {
            let project = candidates.remove(pos);
            dbg.log(&format!(
                "pinned current project: {} (moved from #{})",
                project.display_path(),
                pos
            ));
            candidates.insert(0, project);
        } else {
            dbg.log(&format!(
                "pinned current project: {} (already first)",
                candidates[0].display_path()
            ));
        }
        return false;
    }

    let mut dummy_dbg = crate::debug::DebugLog::new(false);
    let class = project::classify_project(&git_root, Some(root), known_domains, &mut dummy_dbg);

    let (project, is_non_domain) = match class {
        project::ProjectClass::Domain { project, .. } => (project, false),
        project::ProjectClass::NonDomain { project } => (project, true),
    };

    dbg.log(&format!(
        "pinned current project: {} (added)",
        project.display_path()
    ));
    candidates.insert(0, project);
    is_non_domain
}

pub fn build_display_candidates(
    projects: &[Project],
    registry: &AliasRegistry,
) -> Vec<DisplayCandidate> {
    let mut candidates = Vec::new();

    for (idx, project) in projects.iter().enumerate() {
        let full_form = project.display_path();
        candidates.push(DisplayCandidate {
            project_index: idx,
            display_text: full_form.clone(),
            is_alias: false,
            alias_source_path: None,
            disambiguation: None,
        });

        if let Some(alias_entry) = registry.find_nearest_alias(&project.path) {
            let alias_form =
                build_alias_form(&full_form, &alias_entry.source_path, &alias_entry.alias);
            if alias_form != full_form {
                candidates.push(DisplayCandidate {
                    project_index: idx,
                    display_text: alias_form,
                    is_alias: true,
                    alias_source_path: Some(alias_entry.source_path.clone()),
                    disambiguation: None,
                });
            }
        }
    }

    candidates
}

fn build_alias_form(full_form: &str, source_path: &str, alias: &str) -> String {
    let remaining = full_form.strip_prefix(source_path).unwrap_or(full_form);
    if remaining.is_empty() {
        alias.to_string()
    } else {
        format!("{alias}{remaining}")
    }
}

pub fn detect_collisions(candidates: &mut [DisplayCandidate]) {
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, c) in candidates.iter().enumerate() {
        groups
            .entry(c.display_text.to_lowercase())
            .or_default()
            .push(i);
    }

    for indices in groups.values() {
        let project_indices: Vec<usize> = indices
            .iter()
            .map(|&i| candidates[i].project_index)
            .collect();

        let mut unique_projects = project_indices.clone();
        unique_projects.sort();
        unique_projects.dedup();
        if unique_projects.len() <= 1 {
            continue;
        }

        for &i in indices {
            if candidates[i].is_alias {
                if let Some(source) = candidates[i].alias_source_path.clone() {
                    candidates[i].disambiguation = Some(format!("({source})"));
                }
            }
        }
    }
}

pub fn dedup_filter_matches(
    display_candidates: &[DisplayCandidate],
    matches: &[filter::FilterMatch],
) -> Vec<filter::FilterMatch> {
    let mut best: HashMap<usize, usize> = HashMap::new();

    for (match_idx, fm) in matches.iter().enumerate() {
        let dc = &display_candidates[fm.index];
        let project_index = dc.project_index;

        match best.get(&project_index) {
            Some(&existing_match_idx) => {
                let existing_dc = &display_candidates[matches[existing_match_idx].index];
                if should_prefer(&dc.display_text, &existing_dc.display_text) {
                    best.insert(project_index, match_idx);
                }
            }
            None => {
                best.insert(project_index, match_idx);
            }
        }
    }

    let mut result_indices: Vec<usize> = best.values().copied().collect();
    result_indices.sort();
    result_indices
        .into_iter()
        .map(|i| matches[i].clone())
        .collect()
}

fn should_prefer(new: &str, existing: &str) -> bool {
    match new.len().cmp(&existing.len()) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => {
            let new_lower = new.to_lowercase();
            let existing_lower = existing.to_lowercase();
            match new_lower.cmp(&existing_lower) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Greater => false,
                // Case-insensitive tie: prefer the lowercase variant for determinism
                std::cmp::Ordering::Equal => new < existing,
            }
        }
    }
}

fn log_scoring(
    dbg: &mut DebugLog,
    projects: &[Project],
    display_candidates: &[DisplayCandidate],
    result: &select::SelectResult,
) {
    let display_texts: Vec<String> = display_candidates
        .iter()
        .map(|c| c.display_text.clone())
        .collect();
    let filtered = filter::filter_candidates(&display_texts, &result.final_query);
    let deduped = dedup_filter_matches(display_candidates, &filtered);
    if deduped.is_empty() {
        return;
    }

    let mut scored: Vec<(usize, String, f64, f64)> = deduped
        .iter()
        .map(|fm| {
            let dc = &display_candidates[fm.index];
            let s = score::score(&projects[dc.project_index], &result.final_query);
            (
                dc.project_index,
                dc.display_text.clone(),
                s.project_score,
                s.group_score,
            )
        })
        .collect();

    scored.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.1.cmp(&b.1))
    });

    dbg.log(&format!("scoring ({} candidates):", scored.len()));
    for (pi, display, ps, gs) in &scored {
        let marker = if *pi == result.index { "[*]" } else { "   " };
        dbg.log_indent(&format!(
            "{marker} {:<20} project={ps:.2}  group={gs:.2}",
            display
        ));
    }
}

#[cfg(test)]
mod tests {
    // Legacy helper kept for backward-compat tests of display_path output
    fn build_display_items(matches: &[Project]) -> Vec<String> {
        matches.iter().map(|p| p.display_path()).collect()
    }

    use super::*;
    use crate::project::AliasEntry;
    use std::path::PathBuf;

    fn make_project(domain: &str, groups: &[&str], name: &str, path: &str) -> Project {
        Project {
            domain: domain.into(),
            groups: groups.iter().map(|s| s.to_string()).collect(),
            name: name.into(),
            path: PathBuf::from(path),
        }
    }

    fn make_registry(entries: Vec<(&str, &str, &str)>) -> AliasRegistry {
        let mut registry = AliasRegistry::new();
        for (dir, source, alias) in entries {
            registry.add(AliasEntry {
                dir_path: PathBuf::from(dir),
                source_path: source.to_string(),
                alias: alias.to_string(),
            });
        }
        registry
    }

    #[test]
    fn build_display_items_same_domain_includes_prefix() {
        let matches = vec![
            make_project(
                "github.com",
                &["org"],
                "repo-a",
                "/code/github.com/org/repo-a",
            ),
            make_project(
                "github.com",
                &["org"],
                "repo-b",
                "/code/github.com/org/repo-b",
            ),
        ];
        let items = build_display_items(&matches);
        assert_eq!(items[0], "github.com/org/repo-a");
        assert_eq!(items[1], "github.com/org/repo-b");
    }

    #[test]
    fn build_display_items_mixed_domains_includes_prefix() {
        let matches = vec![
            make_project(
                "github.com",
                &["org"],
                "repo-a",
                "/code/github.com/org/repo-a",
            ),
            make_project(
                "gitlab.com",
                &["team"],
                "repo-b",
                "/code/gitlab.com/team/repo-b",
            ),
        ];
        let items = build_display_items(&matches);
        assert_eq!(items[0], "github.com/org/repo-a");
        assert_eq!(items[1], "gitlab.com/team/repo-b");
    }

    #[test]
    fn build_display_items_single_match_includes_domain() {
        let matches = vec![make_project(
            "github.com",
            &[],
            "my-repo",
            "/code/github.com/my-repo",
        )];
        let items = build_display_items(&matches);
        assert_eq!(items[0], "github.com/my-repo");
    }

    #[test]
    fn build_display_items_no_groups_same_domain() {
        let matches = vec![
            make_project("github.com", &[], "foo", "/code/github.com/foo"),
            make_project("github.com", &[], "bar", "/code/github.com/bar"),
        ];
        let items = build_display_items(&matches);
        assert_eq!(items[0], "github.com/foo");
        assert_eq!(items[1], "github.com/bar");
    }

    #[test]
    fn display_candidate_alias_form() {
        let projects = vec![make_project(
            "git.example.com",
            &["backend"],
            "api-gateway",
            "/code/git.example.com/backend/api-gateway",
        )];
        let registry = make_registry(vec![("/code/git.example.com", "git.example.com", "work")]);

        let candidates = build_display_candidates(&projects, &registry);
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].display_text,
            "git.example.com/backend/api-gateway"
        );
        assert!(!candidates[0].is_alias);
        assert_eq!(candidates[1].display_text, "work/backend/api-gateway");
        assert!(candidates[1].is_alias);
    }

    #[test]
    fn display_candidate_nearest_alias() {
        let projects = vec![make_project(
            "git.example.com",
            &["backend", "platform"],
            "user-service",
            "/code/git.example.com/backend/platform/user-service",
        )];
        let registry = make_registry(vec![
            ("/code/git.example.com", "git.example.com", "work"),
            (
                "/code/git.example.com/backend",
                "git.example.com/backend",
                "be",
            ),
        ]);

        let candidates = build_display_candidates(&projects, &registry);
        let alias_candidate = candidates.iter().find(|c| c.is_alias).unwrap();
        assert_eq!(alias_candidate.display_text, "be/platform/user-service");
    }

    #[test]
    fn dedup_both_match_picks_shorter() {
        let projects = vec![make_project(
            "git.example.com",
            &["backend"],
            "api-gateway",
            "/code/git.example.com/backend/api-gateway",
        )];
        let registry = make_registry(vec![("/code/git.example.com", "git.example.com", "work")]);

        let dc = build_display_candidates(&projects, &registry);
        let display_texts: Vec<String> = dc.iter().map(|c| c.display_text.clone()).collect();

        let matches = filter::filter_candidates(&display_texts, "api");
        assert_eq!(matches.len(), 2);

        let deduped = dedup_filter_matches(&dc, &matches);
        assert_eq!(deduped.len(), 1);
        assert_eq!(
            dc[deduped[0].index].display_text,
            "work/backend/api-gateway"
        );
    }

    #[test]
    fn dedup_only_one_matches() {
        let projects = vec![make_project(
            "git.example.com",
            &["backend"],
            "api-gateway",
            "/code/git.example.com/backend/api-gateway",
        )];
        let registry = make_registry(vec![("/code/git.example.com", "git.example.com", "work")]);

        let dc = build_display_candidates(&projects, &registry);
        let display_texts: Vec<String> = dc.iter().map(|c| c.display_text.clone()).collect();

        let matches = filter::filter_candidates(&display_texts, "work api");
        assert_eq!(matches.len(), 1);

        let deduped = dedup_filter_matches(&dc, &matches);
        assert_eq!(deduped.len(), 1);
        assert_eq!(
            dc[deduped[0].index].display_text,
            "work/backend/api-gateway"
        );
    }

    #[test]
    fn dedup_same_length_picks_lexicographic_case_insensitive() {
        // "alpha" < "beta" case-insensitively, so alpha is preferred
        let text_a = "Alpha/project";
        let text_b = "beta_/project";
        assert!(should_prefer(text_a, text_b));
        assert!(!should_prefer(text_b, text_a));

        // When case-insensitively equal, raw comparison is used for determinism
        let text_c = "abcde/project";
        let text_d = "ABCDE/project";
        // "ABCDE" < "abcde" in raw comparison (uppercase before lowercase in ASCII)
        assert!(should_prefer(text_d, text_c));
        assert!(!should_prefer(text_c, text_d));
    }

    #[test]
    fn collision_adds_disambiguation_suffix() {
        let projects = vec![
            make_project(
                "git.example.com",
                &["backend"],
                "api-gateway",
                "/code/git.example.com/backend/api-gateway",
            ),
            make_project(
                "gitlab.com",
                &["backend"],
                "api-gateway",
                "/code/gitlab.com/backend/api-gateway",
            ),
        ];
        let registry = make_registry(vec![
            ("/code/git.example.com", "git.example.com", "work"),
            ("/code/gitlab.com", "gitlab.com", "work"),
        ]);

        let mut dc = build_display_candidates(&projects, &registry);
        detect_collisions(&mut dc);

        let alias_candidates: Vec<&DisplayCandidate> = dc.iter().filter(|c| c.is_alias).collect();
        assert_eq!(alias_candidates.len(), 2);
    }

    #[test]
    fn collision_full_form_no_suffix() {
        let projects = vec![make_project(
            "git.example.com",
            &["backend"],
            "api-gateway",
            "/code/git.example.com/backend/api-gateway",
        )];
        let registry = make_registry(vec![("/code/git.example.com", "git.example.com", "work")]);

        let mut dc = build_display_candidates(&projects, &registry);
        detect_collisions(&mut dc);

        let full_form = dc.iter().find(|c| !c.is_alias).unwrap();
        assert!(full_form.disambiguation.is_none());
    }

    #[test]
    fn no_collision_no_suffix() {
        let projects = vec![
            make_project(
                "git.example.com",
                &["backend"],
                "api-gateway",
                "/code/git.example.com/backend/api-gateway",
            ),
            make_project(
                "gitlab.com",
                &["devops"],
                "deploy-tool",
                "/code/gitlab.com/devops/deploy-tool",
            ),
        ];
        let registry = make_registry(vec![
            ("/code/git.example.com", "git.example.com", "work"),
            ("/code/gitlab.com", "gitlab.com", "work"),
        ]);

        let mut dc = build_display_candidates(&projects, &registry);
        detect_collisions(&mut dc);

        for c in &dc {
            assert!(
                c.disambiguation.is_none(),
                "no collision expected for: {}",
                c.display_text
            );
        }
    }

    #[test]
    fn collision_case_insensitive() {
        let projects = vec![
            make_project(
                "git.example.com",
                &["backend"],
                "api-gateway",
                "/code/git.example.com/backend/api-gateway",
            ),
            make_project(
                "gitlab.com",
                &["backend"],
                "api-gateway",
                "/code/gitlab.com/backend/api-gateway",
            ),
        ];
        let registry = make_registry(vec![
            ("/code/git.example.com", "git.example.com", "Work"),
            ("/code/gitlab.com", "gitlab.com", "work"),
        ]);

        let mut dc = build_display_candidates(&projects, &registry);
        detect_collisions(&mut dc);

        let alias_candidates: Vec<&DisplayCandidate> = dc.iter().filter(|c| c.is_alias).collect();
        assert_eq!(alias_candidates.len(), 2);
        assert_eq!(alias_candidates[0].display_text, "Work/backend/api-gateway");
        assert_eq!(alias_candidates[1].display_text, "work/backend/api-gateway");
    }

    #[test]
    fn alias_form_at_project_level() {
        assert_eq!(
            build_alias_form("domain/group/project", "domain/group/project", "myalias"),
            "myalias"
        );
    }

    #[test]
    fn alias_form_at_domain_level() {
        assert_eq!(
            build_alias_form(
                "git.example.com/backend/api-gateway",
                "git.example.com",
                "work"
            ),
            "work/backend/api-gateway"
        );
    }

    #[test]
    fn alias_form_at_group_level() {
        assert_eq!(
            build_alias_form(
                "git.example.com/backend/api-gateway",
                "git.example.com/backend",
                "be"
            ),
            "be/api-gateway"
        );
    }

    #[test]
    fn non_domain_alias_form() {
        let home = dirs::home_dir().expect("home dir required for test");
        let project_path = home.join("personal/side-project");
        let alias_dir = home.join("personal");
        let source_display = format!("~/{}", "personal");

        let projects = vec![Project {
            domain: String::new(),
            groups: Vec::new(),
            name: "side-project".into(),
            path: project_path,
        }];
        let registry = make_registry(vec![(alias_dir.to_str().unwrap(), &source_display, "me")]);

        let candidates = build_display_candidates(&projects, &registry);
        let alias_candidate = candidates.iter().find(|c| c.is_alias);
        assert!(alias_candidate.is_some());
        assert_eq!(alias_candidate.unwrap().display_text, "me/side-project");
    }
}
