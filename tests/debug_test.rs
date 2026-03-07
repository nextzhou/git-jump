mod common;

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{setup_project_root, setup_project_root_with_config};
use tempfile::TempDir;

// -- Debug flag tests --

#[test]
fn test_debug_does_not_affect_stdout() {
    let tmp = setup_project_root();

    let output_normal = cargo_bin_cmd!("git-jump")
        .args(["jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    let output_debug = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output_normal.status.success());
    assert!(output_debug.status.success());
    assert_eq!(output_normal.stdout, output_debug.stdout);
}

#[test]
fn test_debug_outputs_to_stderr() {
    let tmp = setup_project_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DEBUG: config:"),
        "should contain config info, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG: discovery:"),
        "should contain discovery info, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG: total:"),
        "should contain total timing, got: {stderr}"
    );
}

#[test]
fn test_no_debug_no_stderr_output() {
    let tmp = setup_project_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("DEBUG:"),
        "stderr should not contain DEBUG: when --debug is not used, got: {stderr}"
    );
}

#[test]
fn test_debug_jump_full_output() {
    let tmp = setup_project_root_with_config();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("DEBUG: config:"), "Environment & Config");
    assert!(stderr.contains("DEBUG:   root:"), "root with source");
    assert!(stderr.contains("DEBUG: known domains:"), "known domains");
    assert!(stderr.contains("DEBUG: discovery:"), "Project Discovery");
    assert!(stderr.contains("DEBUG: candidates:"), "candidates");
    assert!(stderr.contains("DEBUG: selection:"), "selection mode");
    assert!(stderr.contains("DEBUG: selected:"), "selected project");
    assert!(stderr.contains("DEBUG: config chain:"), "config chain");
    assert!(
        stderr.contains("(found)"),
        "config chain should show found status"
    );
    assert!(stderr.contains("DEBUG: shell commands:"), "shell commands");
    assert!(stderr.contains("DEBUG: total:"), "total timing");
}

#[test]
fn test_debug_init_outputs_to_stderr() {
    let tmp = TempDir::new().unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "init", "bash"])
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DEBUG: shell: bash (from argument)"),
        "should show shell source, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG: total:"),
        "should show total timing, got: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("gj()"),
        "stdout should still contain script"
    );
}

#[test]
fn test_debug_init_auto_detect_shell_source() {
    let tmp = TempDir::new().unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "init"])
        .env("SHELL", "/usr/bin/zsh")
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DEBUG: shell: zsh (from $SHELL)"),
        "should show auto-detected shell, got: {stderr}"
    );
}

#[test]
fn test_debug_completions_outputs_to_stderr() {
    let tmp = setup_project_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "completions", "bash", "alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DEBUG: discovery:"),
        "should contain discovery, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG: candidates:"),
        "should contain candidates, got: {stderr}"
    );
    assert!(
        !stderr.contains("DEBUG: selection:"),
        "completions should not have selection, got: {stderr}"
    );
}

#[test]
fn test_debug_clone_existing_outputs_to_stderr() {
    let tmp = setup_project_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "clone", "https://example.com/team/project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DEBUG: parsed repo:"),
        "should contain parsed repo, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG: target:"),
        "should contain target, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG: target exists: true"),
        "should show target exists, got: {stderr}"
    );
    assert!(
        stderr.contains("target directory exists, skipping clone"),
        "should note skipped clone, got: {stderr}"
    );
}

#[test]
fn test_debug_error_still_shows_debug_output() {
    let tmp = setup_project_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "nonexistent"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DEBUG: config:"),
        "debug info should appear even on error, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG: total:"),
        "total timing should appear even on error, got: {stderr}"
    );
    assert!(
        stderr.contains("no project matching"),
        "error message should still appear, got: {stderr}"
    );
}

#[test]
fn test_debug_config_chain_shows_not_found() {
    let tmp = setup_project_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("(not found)"),
        "config chain should show not found for missing configs, got: {stderr}"
    );
}

#[test]
fn test_debug_path_abbreviation() {
    let tmp = setup_project_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let home = dirs::home_dir().unwrap();
    let tmp_str = tmp.path().to_str().unwrap();
    if tmp_str.starts_with(home.to_str().unwrap()) {
        assert!(
            stderr.contains("~/"),
            "paths under $HOME should use ~ abbreviation, got: {stderr}"
        );
    }
}

// -- Alias debug tests (AC-15) --

fn setup_debug_alias_root() -> TempDir {
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

fn setup_debug_collision_root() -> TempDir {
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
fn test_debug_shows_aliases() {
    let tmp = setup_debug_alias_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "api-gateway"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("aliases:"),
        "debug output should contain aliases section, got: {stderr}"
    );
    assert!(
        stderr.contains("\"work\""),
        "debug output should show alias value, got: {stderr}"
    );
}

#[test]
fn test_debug_shows_collisions() {
    let tmp = setup_debug_collision_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "work", "api"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("display collisions:"),
        "debug should show collision info, got: {stderr}"
    );
    assert!(
        stderr.contains("work/backend/api-gateway"),
        "collision info should include colliding display text, got: {stderr}"
    );
}
