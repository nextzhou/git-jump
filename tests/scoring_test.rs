use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::TempDir;

// -- Match Scoring tests (AC-1 through AC-14) --

fn setup_scoring_root(domain: &str, projects: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), format!("{domain}\n")).unwrap();

    for &(group, name) in projects {
        let path = root.join(format!("{domain}/{group}/{name}/.git"));
        fs::create_dir_all(&path).unwrap();
    }

    tmp
}

fn setup_multi_domain_root(entries: &[(&str, &str, &str)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();

    let mut domains: Vec<&str> = entries.iter().map(|&(d, _, _)| d).collect();
    domains.sort();
    domains.dedup();
    fs::write(config_dir.join("domains"), domains.join("\n")).unwrap();

    for &(domain, group, name) in entries {
        let path = root.join(format!("{domain}/{group}/{name}/.git"));
        fs::create_dir_all(&path).unwrap();
    }

    tmp
}

fn completions_output(tmp: &TempDir, args: &[&str]) -> String {
    let mut cmd_args = vec!["completions", "bash"];
    cmd_args.extend_from_slice(args);
    let output = cargo_bin_cmd!("git-jump")
        .args(&cmd_args)
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();
    assert!(output.status.success(), "completions failed: {:?}", output);
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn jump_selected_name(tmp: &TempDir, args: &[&str]) -> String {
    let mut cmd_args = vec!["jump"];
    cmd_args.extend_from_slice(args);
    let output = cargo_bin_cmd!("git-jump")
        .args(&cmd_args)
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();
    assert!(output.status.success(), "jump failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|l| l.starts_with("builtin cd"))
        .unwrap_or("")
        .to_string()
}

#[test]
fn test_ac1_exact_name_match_ranks_first() {
    let tmp = setup_scoring_root(
        "d.com",
        &[("org", "foo"), ("org", "foo-bar"), ("org", "foo-baz")],
    );

    let out = completions_output(&tmp, &["foo"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(
        lines[0].ends_with("foo"),
        "first should be foo, got: {lines:?}"
    );
    assert!(
        lines[1].ends_with("foo-bar"),
        "second should be foo-bar, got: {lines:?}"
    );
    assert!(
        lines[2].ends_with("foo-baz"),
        "third should be foo-baz, got: {lines:?}"
    );
}

#[test]
fn test_ac1_exact_match_selected_by_jump() {
    let tmp = setup_scoring_root(
        "d.com",
        &[("org", "foo"), ("org", "foo-bar"), ("org", "foo-baz")],
    );
    let cd = jump_selected_name(&tmp, &["foo"]);
    assert!(cd.contains("/foo'"), "should select foo, got: {cd}");
    assert!(
        !cd.contains("foo-bar") && !cd.contains("foo-baz"),
        "should not select foo-bar/foo-baz, got: {cd}"
    );
}

#[test]
fn test_ac2_higher_coverage_ranks_above() {
    let tmp = setup_scoring_root(
        "d.com",
        &[
            ("org", "platform"),
            ("org", "platform-tools"),
            ("org", "data-platform-v2"),
        ],
    );
    let out = completions_output(&tmp, &["platform"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(
        lines[0].ends_with("platform"),
        "first should be platform, got: {lines:?}"
    );
    assert!(
        lines[1].ends_with("platform-tools"),
        "second should be platform-tools, got: {lines:?}"
    );
    assert!(
        lines[2].ends_with("data-platform-v2"),
        "third should be data-platform-v2, got: {lines:?}"
    );
}

#[test]
fn test_ac3_project_name_match_ranks_above_group() {
    let tmp = setup_scoring_root(
        "d.com",
        &[("backend", "api-gateway"), ("api-team", "deploy-tool")],
    );
    let out = completions_output(&tmp, &["api"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].contains("api-gateway"),
        "project name match first, got: {lines:?}"
    );
    assert!(
        lines[1].contains("deploy-tool"),
        "group-only match second, got: {lines:?}"
    );
}

#[test]
fn test_ac4_multi_token_scoring() {
    let tmp = setup_scoring_root("d.com", &[("team", "my-api"), ("team", "api-dashboard")]);
    let out = completions_output(&tmp, &["team api"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].contains("my-api"),
        "higher coverage first, got: {lines:?}"
    );
    assert!(
        lines[1].contains("api-dashboard"),
        "lower coverage second, got: {lines:?}"
    );
}

#[test]
fn test_ac5_same_score_preserves_alphabetical() {
    let tmp = setup_scoring_root("d.com", &[("org", "foo-bar"), ("org", "foo-baz")]);
    let out = completions_output(&tmp, &["foo"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("foo-bar"), "alpha first, got: {lines:?}");
    assert!(
        lines[1].ends_with("foo-baz"),
        "alpha second, got: {lines:?}"
    );
}

#[test]
fn test_ac7_non_tty_picks_highest_scored() {
    let tmp = setup_scoring_root(
        "d.com",
        &[("org", "foo"), ("org", "foo-bar"), ("org", "foo-baz")],
    );
    let cd = jump_selected_name(&tmp, &["foo"]);
    assert!(
        cd.contains("/foo'"),
        "non-tty should pick highest scored (foo), got: {cd}"
    );
}

#[test]
fn test_ac8_completions_sorted_by_score() {
    let tmp = setup_scoring_root(
        "d.com",
        &[("org", "foo"), ("org", "foo-bar"), ("org", "foo-baz")],
    );
    let out = completions_output(&tmp, &["foo"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("foo"), "completions sorted by score");
}

#[test]
fn test_ac9_filter_logic_unchanged() {
    let tmp = setup_scoring_root(
        "d.com",
        &[
            ("org", "api-gateway"),
            ("org", "api-service"),
            ("team", "web-gateway"),
        ],
    );
    let out = completions_output(&tmp, &["api gate"]);
    let lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "multi-token AND logic: only api-gateway, got: {lines:?}"
    );
    assert!(lines[0].contains("api-gateway"));
}

#[test]
fn test_ac10_domain_only_match_ranks_last() {
    let tmp = setup_multi_domain_root(&[
        ("github.com", "org", "api-gateway"),
        ("api.internal.com", "team", "core-service"),
    ]);

    let output = cargo_bin_cmd!("git-jump")
        .args(["completions", "bash", "api"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2, "both should match, got: {lines:?}");
    assert!(
        lines[0].contains("api-gateway"),
        "project name match ranks first, got: {lines:?}"
    );
    assert!(
        lines[1].contains("core-service"),
        "domain-only match ranks last, got: {lines:?}"
    );
}

#[test]
fn test_ac11_empty_query_preserves_alphabetical() {
    let tmp = setup_scoring_root(
        "d.com",
        &[("org", "beta"), ("org", "alpha"), ("org", "gamma")],
    );
    let out = completions_output(&tmp, &[]);
    let lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("alpha"), "alpha first, got: {lines:?}");
    assert!(lines[1].ends_with("beta"), "beta second, got: {lines:?}");
    assert!(lines[2].ends_with("gamma"), "gamma third, got: {lines:?}");
}

#[test]
fn test_ac12_group_score_breaks_project_tie() {
    let tmp = setup_scoring_root(
        "d.com",
        &[("api", "api-service"), ("backend", "api-service")],
    );
    let out = completions_output(&tmp, &["api"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].contains("api/api-service"),
        "group match breaks tie, got: {lines:?}"
    );
    assert!(
        lines[1].contains("backend/api-service"),
        "no group match second, got: {lines:?}"
    );
}

#[test]
fn test_ac13_multi_level_group_averaging() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "d.com\n").unwrap();

    // alpha/beta/my-tool (groups=["alpha","beta"])
    fs::create_dir_all(root.join("d.com/alpha/beta/my-tool/.git")).unwrap();
    // beta/my-tool (groups=["beta"])
    fs::create_dir_all(root.join("d.com/beta/my-tool/.git")).unwrap();

    let out = completions_output(&tmp, &["beta my"]);
    let lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].contains("beta/my-tool"),
        "single group with higher avg first, got: {lines:?}"
    );
    assert!(
        lines[1].contains("alpha/"),
        "multi-group with lower avg second, got: {lines:?}"
    );
}

#[test]
fn test_ac14_token_slash_splitting() {
    let tmp = setup_scoring_root("d.com", &[("backend", "api-gateway")]);

    let out = completions_output(&tmp, &["backend/api"]);
    let lines: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("api-gateway"));
}

#[test]
fn test_debug_scoring_output() {
    let tmp = setup_scoring_root("d.com", &[("org", "foo"), ("org", "foo-bar")]);

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "foo"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("scoring ("),
        "debug should contain scoring section, got: {stderr}"
    );
    assert!(
        stderr.contains("project="),
        "debug should contain project scores, got: {stderr}"
    );
    assert!(
        stderr.contains("group="),
        "debug should contain group scores, got: {stderr}"
    );
    assert!(
        stderr.contains("[*]"),
        "debug should mark selected item, got: {stderr}"
    );
}
