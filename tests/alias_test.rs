use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::TempDir;

// -- Setup helpers --

fn setup_alias_root() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "git.example.com\n").unwrap();

    let domain_dir = root.join("git.example.com");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(domain_dir.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();

    fs::create_dir_all(root.join("git.example.com/backend/api-gateway/.git")).unwrap();
    fs::create_dir_all(root.join("git.example.com/backend/api-service/.git")).unwrap();

    tmp
}

fn setup_alias_root_with_no_alias_domain() -> TempDir {
    let tmp = setup_alias_root();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::write(config_dir.join("domains"), "git.example.com\ngithub.com\n").unwrap();
    fs::create_dir_all(root.join("github.com/personal/dotfiles/.git")).unwrap();

    tmp
}

fn setup_multi_level_alias_root() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "git.example.com\n").unwrap();

    let domain_dir = root.join("git.example.com");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(domain_dir.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();

    let group_dir = domain_dir.join("backend");
    fs::create_dir_all(&group_dir).unwrap();
    fs::write(group_dir.join(".git-jump.toml"), "alias = \"be\"\n").unwrap();

    fs::create_dir_all(root.join("git.example.com/backend/api-gateway/.git")).unwrap();

    tmp
}

fn setup_collision_root() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "git.example.com\ngitlab.com\n").unwrap();

    let domain1 = root.join("git.example.com");
    fs::create_dir_all(&domain1).unwrap();
    fs::write(domain1.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();
    fs::create_dir_all(root.join("git.example.com/backend/api-gateway/.git")).unwrap();

    let domain2 = root.join("gitlab.com");
    fs::create_dir_all(&domain2).unwrap();
    fs::write(domain2.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();
    fs::create_dir_all(root.join("gitlab.com/backend/api-gateway/.git")).unwrap();

    tmp
}

fn setup_cross_domain_alias_root() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(
        config_dir.join("domains"),
        "git.example.com\ngitlab.internal.com\n",
    )
    .unwrap();

    let domain1 = root.join("git.example.com");
    fs::create_dir_all(&domain1).unwrap();
    fs::write(domain1.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();
    fs::create_dir_all(root.join("git.example.com/backend/api-gateway/.git")).unwrap();

    let domain2 = root.join("gitlab.internal.com");
    fs::create_dir_all(&domain2).unwrap();
    fs::write(domain2.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();
    fs::create_dir_all(root.join("gitlab.internal.com/ops/deploy-tool/.git")).unwrap();

    tmp
}

fn completions_output(tmp: &TempDir, partial: &str) -> String {
    let mut cmd_args = vec!["completions", "bash"];
    if !partial.is_empty() {
        cmd_args.push(partial);
    }
    let output = cargo_bin_cmd!("git-jump")
        .args(&cmd_args)
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();
    assert!(output.status.success(), "completions failed: {:?}", output);
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn jump_cd_line(tmp: &TempDir, args: &[&str]) -> String {
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

fn debug_stderr(tmp: &TempDir, args: &[&str]) -> String {
    let mut cmd_args = vec!["--debug"];
    cmd_args.extend_from_slice(args);
    let output = cargo_bin_cmd!("git-jump")
        .args(&cmd_args)
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stderr).to_string()
}

// -- AC-1: Basic alias match --

#[test]
fn test_alias_single_match_jump() {
    let tmp = setup_alias_root();

    let cd = jump_cd_line(&tmp, &["work", "api-gate"]);
    assert!(
        cd.contains("api-gateway"),
        "should jump to api-gateway via alias form, got: {cd}"
    );
}

// -- AC-3: Full form still matches --

#[test]
fn test_alias_full_form_still_matches() {
    let tmp = setup_alias_root();

    let cd = jump_cd_line(&tmp, &["git.example", "api-gate"]);
    assert!(
        cd.contains("api-gateway"),
        "should match via full form, got: {cd}"
    );
}

// -- AC-4: Dedup picks shorter form --

#[test]
fn test_alias_dedup_both_match_picks_shorter() {
    let tmp = setup_alias_root();

    let out = completions_output(&tmp, "api-gateway");
    let lines: Vec<&str> = out.lines().collect();

    let has_alias_form = lines.iter().any(|l| *l == "work/backend/api-gateway");
    let has_full_form = lines
        .iter()
        .any(|l| *l == "git.example.com/backend/api-gateway");

    assert!(
        has_alias_form,
        "should show shorter alias form, got: {lines:?}"
    );
    assert!(
        !has_full_form,
        "should NOT show full form when alias form is shorter, got: {lines:?}"
    );
}

// -- AC-5: No alias = unaffected --

#[test]
fn test_alias_no_alias_unaffected() {
    let tmp = setup_alias_root_with_no_alias_domain();

    let out = completions_output(&tmp, "dot");
    assert!(
        out.contains("github.com/personal/dotfiles"),
        "no-alias domain should show full form, got: {out}"
    );
    assert!(
        !out.lines()
            .any(|l| l.starts_with("work/") && l.contains("dotfiles")),
        "dotfiles should not have alias form, got: {out}"
    );
    assert!(
        !out.lines()
            .any(|l| l.starts_with("work/") && l.contains("dotfiles")),
        "dotfiles should not have alias form, got: {out}"
    );
}

// -- AC-6: Multi-level alias takes nearest --

#[test]
fn test_alias_multi_level_nearest() {
    let tmp = setup_multi_level_alias_root();

    let out = completions_output(&tmp, "be api");
    assert!(
        out.contains("be/api-gateway"),
        "should use nearest alias (be), got: {out}"
    );
}

// -- AC-7: Outer alias does not override inner --

#[test]
fn test_alias_multi_level_outer_does_not_override_inner() {
    let tmp = setup_multi_level_alias_root();

    let out = completions_output(&tmp, "work api");
    assert!(
        out.is_empty() || !out.contains("api-gateway"),
        "outer alias 'work' should not match when inner alias 'be' overrides, got: {out}"
    );
}

// -- AC-8: Cross-domain same alias aggregation --

#[test]
fn test_alias_cross_domain_grouping() {
    let tmp = setup_cross_domain_alias_root();

    let out = completions_output(&tmp, "work");
    assert!(
        out.contains("work/backend/api-gateway"),
        "should include git.example.com project, got: {out}"
    );
    assert!(
        out.contains("work/ops/deploy-tool"),
        "should include gitlab.internal.com project, got: {out}"
    );
}

// -- AC-9: Collision disambiguation (integration: verify no crash) --

#[test]
fn test_alias_collision_disambiguation() {
    let tmp = setup_collision_root();

    let stderr = debug_stderr(&tmp, &["jump", "work", "api"]);
    assert!(
        stderr.contains("display collisions:"),
        "debug output should show collision info, got: {stderr}"
    );
    assert!(
        stderr.contains("work/backend/api-gateway"),
        "collision info should mention the colliding display text, got: {stderr}"
    );
}

// -- AC-10: Disambiguation suffix not searchable --

#[test]
fn test_alias_disambiguation_suffix_not_searchable() {
    let tmp = setup_collision_root();

    let out = completions_output(&tmp, "git.example api");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "only full form of git.example.com project should match, got: {lines:?}"
    );
    assert!(
        lines[0].contains("git.example.com/backend/api-gateway"),
        "should be the full form, got: {lines:?}"
    );
}

// -- AC-13: Invalid alias value ignored --

#[test]
fn test_alias_invalid_value_ignored() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "git.example.com\n").unwrap();

    let domain_dir = root.join("git.example.com");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(domain_dir.join(".git-jump.toml"), "alias = \"work/test\"\n").unwrap();
    fs::create_dir_all(root.join("git.example.com/team/my-project/.git")).unwrap();

    let stderr = debug_stderr(&tmp, &["jump", "my-project"]);
    assert!(
        stderr.contains("invalid"),
        "debug should mention invalid alias, got: {stderr}"
    );

    let out = completions_output(&tmp, "my-project");
    assert!(
        out.contains("git.example.com/team/my-project"),
        "project should show full form when alias is invalid, got: {out}"
    );
    assert!(
        !out.contains("work/test"),
        "invalid alias should not create alias form, got: {out}"
    );
}

// -- AC-14: Scoring unaffected by alias --

#[test]
fn test_alias_scoring_unaffected() {
    let tmp = setup_alias_root();

    let out = completions_output(&tmp, "api gate");
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines.iter().any(|l| l.contains("api-gateway")),
        "api-gateway should match 'api gate', got: {lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("api-service")),
        "api-service should NOT match 'api gate', got: {lines:?}"
    );

    let out2 = completions_output(&tmp, "api serv");
    let lines2: Vec<&str> = out2.lines().collect();
    assert!(
        lines2.iter().any(|l| l.contains("api-service")),
        "api-service should match 'api serv', got: {lines2:?}"
    );
    assert!(
        !lines2.iter().any(|l| l.contains("api-gateway")),
        "api-gateway should NOT match 'api serv', got: {lines2:?}"
    );
}

// -- AC-19: Case-insensitive collision detection --

#[test]
fn test_alias_case_insensitive_collision() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "git.example.com\ngitlab.com\n").unwrap();

    let domain1 = root.join("git.example.com");
    fs::create_dir_all(&domain1).unwrap();
    fs::write(domain1.join(".git-jump.toml"), "alias = \"Work\"\n").unwrap();
    fs::create_dir_all(root.join("git.example.com/backend/api-gateway/.git")).unwrap();

    let domain2 = root.join("gitlab.com");
    fs::create_dir_all(&domain2).unwrap();
    fs::write(domain2.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();
    fs::create_dir_all(root.join("gitlab.com/backend/api-gateway/.git")).unwrap();

    let stderr = debug_stderr(&tmp, &["jump", "work", "api"]);
    assert!(
        stderr.contains("display collisions:"),
        "case-insensitive collision should be detected, got: {stderr}"
    );
}

// -- AC-20: TOML parse error aborts --

#[test]
fn test_alias_toml_parse_error_aborts() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "git.example.com\n").unwrap();

    let domain_dir = root.join("git.example.com");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(domain_dir.join(".git-jump.toml"), "alias = \"work\n").unwrap();
    fs::create_dir_all(root.join("git.example.com/backend/api-gateway/.git")).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "api"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "should fail on TOML parse error, but succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("toml")
            || stderr.to_lowercase().contains("parse")
            || stderr.to_lowercase().contains("error"),
        "should mention parse error, got: {stderr}"
    );
}

// -- AC-21: gj . <tokens> dot expansion ignores alias --

#[test]
fn test_alias_dot_expansion_ignores_alias() {
    let tmp = setup_alias_root();
    let cwd = tmp.path().join("git.example.com/backend");

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", ".", "api"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&cwd)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "dot expansion + api should match, got: {:?}",
        output
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("api-gateway"),
        "should match api-gateway (dot expands to 'backend', not alias 'work'), got: {stdout}"
    );
}
