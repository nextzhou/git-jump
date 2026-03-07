mod common;

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{setup_project_root, setup_project_root_with_config};
use tempfile::TempDir;

#[test]
fn test_jump_single_match() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cd"));
    assert!(stdout.contains("project-alpha"));
}

#[test]
fn test_jump_no_match() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "nonexistent"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no project matching"));
}

#[test]
fn test_jump_with_env_and_git_config() {
    let tmp = setup_project_root_with_config();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cd"));
    assert!(stdout.contains("export GOPATH="));
    assert!(stdout.contains("git config"));
    assert!(stdout.contains("user.name"));
}

#[test]
fn test_jump_hooks_parent_to_child_order() {
    let tmp = setup_project_root_with_config();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let domain_hook_pos = stdout.find("echo domain-hook");
    let project_hook_pos = stdout.find("echo project-hook");
    assert!(domain_hook_pos.is_some());
    assert!(project_hook_pos.is_some());
    assert!(domain_hook_pos.unwrap() < project_hook_pos.unwrap());
}

#[test]
fn test_jump_without_config_shows_setup_hint() {
    let tmp = TempDir::new().unwrap();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "anything"])
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gj setup"));
}

// -- Unified Jump Flow tests --

#[test]
fn test_jump_no_args_picks_first_project_non_tty() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cd"));
}

#[test]
fn test_jump_display_text_matching() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "team"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cd"));
}

#[test]
fn test_jump_multi_token_filter() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "team", "alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("project-alpha"));
}

#[test]
fn test_jump_no_match_with_multi_token() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "zzzzz", "xxxxx"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no project matching"));
}

#[test]
fn test_jump_clone_as_filter_token() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["jump", "clone"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no project matching"));
}

// -- logo_text in jump output --

#[test]
fn test_domain_jump_always_outputs_logo_text() {
    let tmp = setup_project_root_with_config();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export _GIT_JUMP_LOGO_TEXT="),
        "should always have logo_text export, got: {stdout}"
    );
    assert!(
        !stdout.contains("_GIT_JUMP_HAS_CONFIG"),
        "should not have old marker, got: {stdout}"
    );
}

#[test]
fn test_domain_jump_without_config_has_empty_logo_text() {
    let tmp = setup_project_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export _GIT_JUMP_LOGO_TEXT=''"),
        "should have empty logo_text when no config, got: {stdout}"
    );
}

// -- logo_text config integration tests --

#[test]
fn test_jump_with_global_logo_text_fallback() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\nlogo_text = \"Global\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "example.com\n").unwrap();

    fs::create_dir_all(root.join("example.com/team/my-project/.git")).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "my-project"])
        .env("_GIT_JUMP_ROOT", root)
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export _GIT_JUMP_LOGO_TEXT='Global'"),
        "should fallback to global logo_text, got: {stdout}"
    );
}

#[test]
fn test_jump_logo_text_domain_overrides_global() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\nlogo_text = \"Global\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "example.com\n").unwrap();

    let domain_dir = root.join("example.com");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(
        domain_dir.join(".git-jump.toml"),
        "logo_text = \"Domain\"\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("example.com/team/my-project/.git")).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "my-project"])
        .env("_GIT_JUMP_ROOT", root)
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export _GIT_JUMP_LOGO_TEXT='Domain'"),
        "domain should override global, got: {stdout}"
    );
}

#[test]
fn test_jump_logo_text_empty_string_disables() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\nlogo_text = \"Global\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "example.com\n").unwrap();

    let domain_dir = root.join("example.com");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(domain_dir.join(".git-jump.toml"), "logo_text = \"\"\n").unwrap();

    fs::create_dir_all(root.join("example.com/team/my-project/.git")).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "my-project"])
        .env("_GIT_JUMP_ROOT", root)
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export _GIT_JUMP_LOGO_TEXT=''"),
        "empty string should override global, got: {stdout}"
    );
}
