use std::collections::BTreeMap;
use std::time::Instant;

use crate::config::{self, GlobalConfig};
use crate::debug::{self, DebugLog};
use crate::error::Result;
use crate::filter;
use crate::project;
use crate::resolve;
use crate::score;

pub fn run(partial: Option<&str>, global: &GlobalConfig, dbg: &mut DebugLog) -> Result<String> {
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

    let alias_registry = project::load_domain_aliases(&root, &known_domains, dbg)?;

    debug::log_aliases(dbg, &alias_registry);

    let display_candidates = resolve::build_display_candidates(&all_projects, &alias_registry);
    let display_texts: Vec<String> = display_candidates
        .iter()
        .map(|c| c.display_text.clone())
        .collect();

    let query = partial.unwrap_or("");
    let matches = filter::filter_candidates(&display_texts, query);
    let deduped = resolve::dedup_filter_matches(&display_candidates, &matches);

    let mut scored: Vec<(String, f64, f64)> = deduped
        .iter()
        .map(|m| {
            let dc = &display_candidates[m.index];
            let project = &all_projects[dc.project_index];
            let s = score::score(project, query);
            (dc.display_text.clone(), s.project_score, s.group_score)
        })
        .collect();

    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.0.cmp(&b.0))
    });

    if dbg.is_enabled() {
        let match_word = if scored.len() == 1 {
            "match"
        } else {
            "matches"
        };
        dbg.log(&format!(
            "candidates: {} -> {} {match_word}",
            display_texts.len(),
            scored.len()
        ));
    }

    let result: Vec<&str> = scored.iter().map(|(text, _, _)| text.as_str()).collect();
    Ok(result.join("\n"))
}
