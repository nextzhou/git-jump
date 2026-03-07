mod common;

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use common::setup_project_root;
use tempfile::TempDir;

#[test]
fn test_completions_all_projects() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["completions", "bash"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("project-alpha"));
    assert!(stdout.contains("project-beta"));
}

#[test]
fn test_completions_with_partial() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["completions", "bash", "alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("project-alpha"));
    assert!(!stdout.contains("project-beta"));
}

#[test]
fn test_completions_without_config_shows_setup_hint() {
    let tmp = TempDir::new().unwrap();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["completions", "bash"])
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gj setup"));
}

#[test]
fn test_completions_returns_display_text() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["completions", "bash"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("example.com/team/project-alpha"),
        "completions should return display text with domain prefix, got: {stdout}"
    );
}

#[test]
fn test_completions_filters_by_display_text() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["completions", "bash", "team"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("example.com/team/project-alpha"),
        "completions should match display text, got: {stdout}"
    );
    assert!(stdout.contains("example.com/team/project-beta"));
}

// -- Alias completions tests (AC-11, AC-12, AC-18) --

fn setup_completions_alias_root() -> TempDir {
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

    tmp
}

fn setup_completions_collision_root() -> TempDir {
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

    let d1 = root.join("git.example.com");
    fs::create_dir_all(&d1).unwrap();
    fs::write(d1.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();
    fs::create_dir_all(root.join("git.example.com/backend/api-gateway/.git")).unwrap();

    let d2 = root.join("gitlab.com");
    fs::create_dir_all(&d2).unwrap();
    fs::write(d2.join(".git-jump.toml"), "alias = \"work\"\n").unwrap();
    fs::create_dir_all(root.join("gitlab.com/backend/api-gateway/.git")).unwrap();

    tmp
}

#[test]
fn test_completions_include_alias_forms() {
    let tmp = setup_completions_alias_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["completions", "bash", "work"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("work/backend/api-gateway"),
        "completions should include alias form, got: {stdout}"
    );
}

#[test]
fn test_completions_dedup() {
    let tmp = setup_completions_alias_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["completions", "bash", "api"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|l| l.contains("api-gateway"))
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "api-gateway should appear exactly once (deduped), got: {lines:?}"
    );
}

#[test]
fn test_completions_no_disambiguation_suffix() {
    let tmp = setup_completions_collision_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["completions", "bash", "work"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains('('),
        "completions should not contain disambiguation suffix, got: {stdout}"
    );
    assert!(
        stdout.contains("work/backend/api-gateway"),
        "should still contain alias form, got: {stdout}"
    );
}
