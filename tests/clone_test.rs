mod common;

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use common::{setup_project_root, setup_project_root_with_config};
use tempfile::TempDir;

#[test]
fn test_clone_existing_dir_outputs_path() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "https://example.com/team/project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.trim();
    assert!(
        path.ends_with("example.com/team/project-alpha"),
        "should output target path, got: {path}"
    );
    assert!(
        !path.contains("cd"),
        "should not contain cd command, got: {path}"
    );
    assert!(
        !path.contains("export"),
        "should not contain export, got: {path}"
    );
}

#[test]
fn test_clone_existing_dir_no_shell_commands() {
    let tmp = setup_project_root_with_config();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "https://example.com/team/project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.trim();
    assert!(
        path.ends_with("example.com/team/project-alpha"),
        "should output target path, got: {path}"
    );
    assert!(
        !path.contains("export"),
        "should not contain export, got: {path}"
    );
    assert!(
        !path.contains("git config"),
        "should not contain git config, got: {path}"
    );
    assert!(
        !path.contains("echo"),
        "should not contain hooks, got: {path}"
    );
}

#[test]
fn test_clone_rejects_shorthand() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "team/project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid repo URL"),
        "shorthand should be rejected, got: {stderr}"
    );
    assert!(
        stderr.contains("hint:"),
        "should include hint for full URL, got: {stderr}"
    );
}

#[test]
fn test_clone_rejects_single_segment() {
    let tmp = setup_project_root();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "noslash"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid repo URL"),
        "single segment should be rejected, got: {stderr}"
    );
}

#[test]
fn test_clone_without_config_shows_setup_hint() {
    let tmp = TempDir::new().unwrap();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "https://github.com/group/project"])
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gj setup"));
}

#[test]
fn test_clone_creates_domain_config() {
    let tmp = setup_project_root();

    let domain_config = tmp.path().join("example.com/.git-jump.toml");
    assert!(!domain_config.exists(), "precondition: no domain config");

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "https://example.com/team/project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        domain_config.exists(),
        "domain config should be created after clone"
    );

    let content = fs::read_to_string(&domain_config).unwrap();
    assert!(
        content.contains("web_url_template"),
        "generated config should contain web_url_template"
    );
}

#[test]
fn test_clone_domain_config_not_overwritten() {
    let tmp = setup_project_root();
    let domain_dir = tmp.path().join("example.com");
    let domain_config = domain_dir.join(".git-jump.toml");

    let custom_content = "# my custom config\nalias = \"ex\"\n";
    fs::write(&domain_config, custom_content).unwrap();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "https://example.com/team/project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = fs::read_to_string(&domain_config).unwrap();
    assert_eq!(
        content, custom_content,
        "existing domain config should not be overwritten"
    );
}

#[test]
fn test_clone_hint_on_new_domain_config() {
    let tmp = setup_project_root();

    let domain_config = tmp.path().join("example.com/.git-jump.toml");
    assert!(!domain_config.exists());

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "https://example.com/team/project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hint: created domain config"),
        "stderr should contain hint about created config, got: {stderr}"
    );
    assert!(
        stderr.contains("hint: review and customize"),
        "stderr should contain review hint, got: {stderr}"
    );
}

#[test]
fn test_clone_no_hint_when_config_exists() {
    let tmp = setup_project_root();
    let domain_config = tmp.path().join("example.com/.git-jump.toml");
    fs::write(&domain_config, "# existing\n").unwrap();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "https://example.com/team/project-alpha"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("hint:"),
        "stderr should NOT contain hint when config exists, got: {stderr}"
    );
}

#[test]
fn test_clone_domain_config_bitbucket() {
    let tmp = setup_project_root();

    fs::create_dir_all(tmp.path().join("bitbucket.org/team/project/.git")).unwrap();
    let domains_path = tmp.path().join(".config/git-jump/domains");
    let mut domains = fs::read_to_string(&domains_path).unwrap();
    domains.push_str("bitbucket.org\n");
    fs::write(&domains_path, &domains).unwrap();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["clone", "https://bitbucket.org/team/project"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(output.status.success());

    let config_path = tmp.path().join("bitbucket.org/.git-jump.toml");
    assert!(config_path.exists(), "bitbucket domain config should exist");

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("/src/"),
        "bitbucket config should use /src/ format, got: {content}"
    );

    let has_active_template = content
        .lines()
        .any(|l| l.starts_with("web_url_template") && !l.starts_with('#'));
    assert!(
        has_active_template,
        "bitbucket config should have uncommented web_url_template"
    );
}
