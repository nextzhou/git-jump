use crate::project::Project;

#[derive(Debug, Clone, Copy)]
pub struct MatchScore {
    pub project_score: f64,
    pub group_score: f64,
}

impl MatchScore {
    pub fn zero() -> Self {
        Self {
            project_score: 0.0,
            group_score: 0.0,
        }
    }
}

/// Score a project against the given query.
///
/// Tokens are split by whitespace, then each token is further split by `/`
/// into sub-tokens for coverage calculation.  Domain is excluded from scoring.
pub fn score(project: &Project, query: &str) -> MatchScore {
    let tokens: Vec<&str> = query.split_whitespace().collect();
    score_with_tokens(project, &tokens)
}

pub fn score_with_tokens(project: &Project, tokens: &[&str]) -> MatchScore {
    if tokens.is_empty() {
        return MatchScore::zero();
    }

    // Split tokens by "/" into sub-tokens for scoring.
    // Filtering still uses the original token as a whole string.
    let sub_tokens: Vec<String> = tokens
        .iter()
        .flat_map(|t| t.split('/'))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect();

    if sub_tokens.is_empty() {
        return MatchScore::zero();
    }

    let project_score = coverage(&project.name, &sub_tokens);

    let group_score = if project.groups.is_empty() {
        0.0
    } else {
        let sum: f64 = project
            .groups
            .iter()
            .map(|g| coverage(g, &sub_tokens))
            .sum();
        sum / project.groups.len() as f64
    };

    MatchScore {
        project_score,
        group_score,
    }
}

/// sum(matching sub-token byte-lengths) / segment byte-length.
fn coverage(segment: &str, lower_sub_tokens: &[String]) -> f64 {
    if segment.is_empty() {
        return 0.0;
    }
    let lower_segment = segment.to_lowercase();
    let matched_len: usize = lower_sub_tokens
        .iter()
        .filter(|t| lower_segment.contains(t.as_str()))
        .map(|t| t.len())
        .sum();
    matched_len as f64 / segment.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn project(domain: &str, groups: &[&str], name: &str) -> Project {
        let groups: Vec<String> = groups.iter().map(|s| s.to_string()).collect();
        Project {
            domain: domain.into(),
            groups,
            name: name.into(),
            path: PathBuf::from(format!("/code/{domain}/{name}")),
        }
    }

    // -- Example 1: prefix conflict --

    #[test]
    fn example1_exact_match_scores_highest() {
        let foo = project("d", &["org"], "foo");
        let foo_bar = project("d", &["org"], "foo-bar");
        let foo_baz = project("d", &["org"], "foo-baz");

        let s1 = score(&foo, "foo");
        let s2 = score(&foo_bar, "foo");
        let s3 = score(&foo_baz, "foo");

        assert!((s1.project_score - 1.0).abs() < f64::EPSILON); // 3/3
        assert!((s2.project_score - 3.0 / 7.0).abs() < 0.01); // 3/7
        assert!((s3.project_score - 3.0 / 7.0).abs() < 0.01); // 3/7
        assert!(s1.project_score > s2.project_score);
    }

    // -- Example 2: project name match > group-only match --

    #[test]
    fn example2_project_name_beats_group() {
        let api_gw = project("d", &["backend"], "api-gateway");
        let deploy = project("d", &["api-team"], "deploy-tool");

        let s1 = score(&api_gw, "api");
        let s2 = score(&deploy, "api");

        assert!((s1.project_score - 3.0 / 11.0).abs() < 0.01); // "api" in "api-gateway"
        assert!((s2.project_score - 0.0).abs() < f64::EPSILON); // no match in "deploy-tool"
        assert!(s1.project_score > s2.project_score);

        assert!((s1.group_score - 0.0).abs() < f64::EPSILON); // "api" not in "backend"
        assert!((s2.group_score - 3.0 / 8.0).abs() < 0.01); // "api" in "api-team"
    }

    // -- Example 3: multi-token + multi-level groups --

    #[test]
    fn example3_multi_token_coverage() {
        let user_svc = project("d", &["backend", "platform"], "user-service");
        let user_tool = project("d", &["platform"], "user-tool");

        let s1 = score(&user_svc, "plat user");
        let s2 = score(&user_tool, "plat user");

        // project_score
        assert!((s1.project_score - 4.0 / 12.0).abs() < 0.01); // "user" in 12-char name
        assert!((s2.project_score - 4.0 / 9.0).abs() < 0.01); // "user" in 9-char name
        assert!(s2.project_score > s1.project_score);

        // group_score
        // user_svc: backend=0, platform="plat" 4/8=0.5 -> avg (0+0.5)/2 = 0.25
        assert!((s1.group_score - 0.25).abs() < 0.01);
        // user_tool: platform="plat" 4/8=0.5 -> avg 0.5/1 = 0.5
        assert!((s2.group_score - 0.5).abs() < 0.01);
    }

    // -- Example 4: coverage gradient --

    #[test]
    fn example4_coverage_gradient() {
        let p1 = project("d", &["org"], "platform");
        let p2 = project("d", &["org"], "platform-tools");
        let p3 = project("d", &["org"], "data-platform-v2");

        let s1 = score(&p1, "platform");
        let s2 = score(&p2, "platform");
        let s3 = score(&p3, "platform");

        assert!((s1.project_score - 1.0).abs() < f64::EPSILON); // 8/8
        assert!((s2.project_score - 8.0 / 14.0).abs() < 0.01); // 8/14
        assert!((s3.project_score - 8.0 / 16.0).abs() < 0.01); // 8/16
        assert!(s1.project_score > s2.project_score);
        assert!(s2.project_score > s3.project_score);
    }

    // -- Empty query --

    #[test]
    fn empty_query_scores_zero() {
        let p = project("d", &["org"], "foo");
        let s = score(&p, "");
        assert!((s.project_score - 0.0).abs() < f64::EPSILON);
        assert!((s.group_score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn whitespace_only_query_scores_zero() {
        let p = project("d", &["org"], "foo");
        let s = score(&p, "   ");
        assert!((s.project_score - 0.0).abs() < f64::EPSILON);
        assert!((s.group_score - 0.0).abs() < f64::EPSILON);
    }

    // -- Token contributes to both project and group --

    #[test]
    fn token_matches_both_name_and_group() {
        let p = project("d", &["api"], "api-service");
        let s = score(&p, "api");

        // "api" matches both name and group independently
        assert!((s.project_score - 3.0 / 11.0).abs() < 0.01);
        assert!((s.group_score - 1.0).abs() < f64::EPSILON); // 3/3
    }

    // -- AC-12: group score breaks project score tie --

    #[test]
    fn group_score_tiebreaker() {
        let p1 = project("d", &["api"], "api-service");
        let p2 = project("d", &["backend"], "api-service");

        let s1 = score(&p1, "api");
        let s2 = score(&p2, "api");

        // Same project_score
        assert!((s1.project_score - s2.project_score).abs() < f64::EPSILON);
        // p1 has higher group_score
        assert!(s1.group_score > s2.group_score);
    }

    // -- AC-13: multi-level group averaging --

    #[test]
    fn multi_level_group_averaging() {
        let p1 = project("d", &["alpha", "beta"], "my-tool");
        let p2 = project("d", &["beta"], "my-tool");

        let s1 = score(&p1, "beta my");
        let s2 = score(&p2, "beta my");

        // Same project_score: "my" in "my-tool" -> 2/7
        assert!((s1.project_score - 2.0 / 7.0).abs() < 0.01);
        assert!((s2.project_score - 2.0 / 7.0).abs() < 0.01);

        // p1 group_score: (0 + 4/4) / 2 = 0.5
        assert!((s1.group_score - 0.5).abs() < 0.01);
        // p2 group_score: 4/4 / 1 = 1.0
        assert!((s2.group_score - 1.0).abs() < f64::EPSILON);

        assert!(s2.group_score > s1.group_score);
    }

    // -- AC-14: token "/" splitting --

    #[test]
    fn token_slash_splitting() {
        let p = project("d", &["backend"], "api-gateway");
        let s = score(&p, "backend/api");

        // "backend/api" splits into sub-tokens "backend" and "api"
        // project_score: "api" matches "api-gateway" -> 3/11
        assert!((s.project_score - 3.0 / 11.0).abs() < 0.01);
        // group_score: "backend" matches "backend" -> 7/7 = 1.0
        assert!((s.group_score - 1.0).abs() < f64::EPSILON);
    }

    // -- Empty groups --

    #[test]
    fn empty_groups_gives_zero_group_score() {
        let p = project("d", &[], "my-project");
        let s = score(&p, "my");
        assert!(s.project_score > 0.0);
        assert!((s.group_score - 0.0).abs() < f64::EPSILON);
    }

    // -- Case insensitivity --

    #[test]
    fn case_insensitive_matching() {
        let p = project("d", &["Backend"], "API-Gateway");
        let s = score(&p, "api");
        assert!(s.project_score > 0.0);
    }

    // -- Overlapping token coverage can exceed 1.0 --

    #[test]
    fn overlapping_tokens_can_exceed_one() {
        let p = project("d", &[], "abc");
        let s = score(&p, "ab bc");
        // "ab"(2) + "bc"(2) both match "abc"(3) -> 4/3 = 1.33
        assert!(s.project_score > 1.0);
        assert!((s.project_score - 4.0 / 3.0).abs() < 0.01);
    }

    // -- AC-4: multi-token scoring --

    #[test]
    fn ac4_multi_token_scoring() {
        let my_api = project("d", &["team"], "my-api");
        let api_dash = project("d", &["team"], "api-dashboard");

        let s1 = score(&my_api, "team api");
        let s2 = score(&api_dash, "team api");

        // Both have same group_score: "team" exact match -> 4/4 = 1.0
        assert!((s1.group_score - 1.0).abs() < f64::EPSILON);
        assert!((s2.group_score - 1.0).abs() < f64::EPSILON);

        // my-api: "api" matches -> 3/6 = 0.5
        assert!((s1.project_score - 0.5).abs() < 0.01);
        // api-dashboard: "api" matches -> 3/13 ~ 0.23
        assert!((s2.project_score - 3.0 / 13.0).abs() < 0.01);

        assert!(s1.project_score > s2.project_score);
    }
}
