use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::TempDir;

// -- Browse command tests --

fn browse_output(tmp: &TempDir, args: &[&str]) -> std::process::Output {
    let mut cmd_args = vec!["browse"];
    cmd_args.extend_from_slice(args);
    cargo_bin_cmd!("git-jump")
        .args(&cmd_args)
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap()
}

fn setup_browse_root() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();

    fs::write(config_dir.join("domains"), "github.com\ngitlab.com\n").unwrap();

    // github.com/org/my-repo/.git
    fs::create_dir_all(root.join("github.com/org/my-repo/.git")).unwrap();
    // github.com/org/api-gateway/.git
    fs::create_dir_all(root.join("github.com/org/api-gateway/.git")).unwrap();
    // github.com/org/api-service/.git
    fs::create_dir_all(root.join("github.com/org/api-service/.git")).unwrap();
    // github.com/org/sub/deep-project/.git (subgroup)
    fs::create_dir_all(root.join("github.com/org/sub/deep-project/.git")).unwrap();
    // github.com/solo-project/.git (no groups)
    fs::create_dir_all(root.join("github.com/solo-project/.git")).unwrap();
    // gitlab.com/org/tool/.git
    fs::create_dir_all(root.join("gitlab.com/org/tool/.git")).unwrap();

    tmp
}

#[test]
fn test_browse_ac1_default_url_inference() {
    let tmp = setup_browse_root();
    let output = browse_output(&tmp, &["my-repo"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://github.com/org/my-repo");
}

#[test]
fn test_browse_ac2_domain_url_template() {
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
    fs::write(
        domain_dir.join(".git-jump.toml"),
        "web_url_template = \"https://{domain}:8443/{groups}/{project}\"\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("git.example.com/backend/api/.git")).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse", "api"])
        .env("_GIT_JUMP_ROOT", root)
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://git.example.com:8443/backend/api");
}

#[test]
fn test_browse_ac3_project_static_template() {
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

    let project_dir = root.join("git.example.com/team/special");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::write(
        project_dir.join(".git-jump.toml"),
        "web_url_template = \"https://custom.example.com/special-project\"\n",
    )
    .unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse", "special"])
        .env("_GIT_JUMP_ROOT", root)
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://custom.example.com/special-project");
}

#[test]
fn test_browse_ac4_project_template_overrides_domain() {
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
    fs::write(
        domain_dir.join(".git-jump.toml"),
        "web_url_template = \"https://{domain}:8443/{groups}/{project}\"\n",
    )
    .unwrap();

    let project_dir = root.join("git.example.com/team/special");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::write(
        project_dir.join(".git-jump.toml"),
        "web_url_template = \"https://override.example.com/special\"\n",
    )
    .unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse", "special"])
        .env("_GIT_JUMP_ROOT", root)
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://override.example.com/special");
}

#[test]
fn test_browse_ac6_browser_failure_exit_zero() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!(
            "root = \"{}\"\nbrowser = \"/nonexistent/command {{url}}\"\n",
            root.display()
        ),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "github.com\n").unwrap();

    fs::create_dir_all(root.join("github.com/org/my-repo/.git")).unwrap();

    // Note: this test runs in pipe mode (stdout not tty), so browser is skipped.
    // The browser failure is only triggered when stdout IS a tty.
    // We still test that the exit code is 0 and URL is output.
    let output = cargo_bin_cmd!("git-jump")
        .args(["browse", "my-repo"])
        .env("_GIT_JUMP_ROOT", root)
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success(), "exit code should be 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("https://github.com/org/my-repo"),
        "URL should still be output, got: {stdout}"
    );
}

#[test]
fn test_browse_ac9_no_match_error() {
    let tmp = setup_browse_root();
    let output = browse_output(&tmp, &["nonexistent-project-xyz"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no project matching"),
        "should report no match, got: {stderr}"
    );
}

#[test]
fn test_browse_ac10_url_pipeable() {
    let tmp = setup_browse_root();
    let output = browse_output(&tmp, &["my-repo"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should be single line URL, got: {lines:?}");
    assert!(lines[0].starts_with("https://"));
}

#[test]
fn test_browse_ac11_subgroup_url() {
    let tmp = setup_browse_root();
    let output = browse_output(&tmp, &["deep-project"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://github.com/org/sub/deep-project");
}

#[test]
fn test_browse_ac12_empty_groups() {
    let tmp = setup_browse_root();
    let output = browse_output(&tmp, &["solo-project"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://github.com/solo-project");
    assert!(
        !stdout.contains("//solo"),
        "should not have double slashes before project name, got: {stdout}"
    );
}

#[test]
fn test_browse_ac13_debug_output() {
    let tmp = setup_browse_root();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "browse", "my-repo"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("url source:"),
        "debug should show URL source, got: {stderr}"
    );
    assert!(
        stderr.contains("url:"),
        "debug should show constructed URL, got: {stderr}"
    );
    assert!(
        stderr.contains("browser:"),
        "debug should show browser info, got: {stderr}"
    );
}

#[test]
fn test_browse_debug_does_not_affect_stdout() {
    let tmp = setup_browse_root();

    let output_normal = browse_output(&tmp, &["my-repo"]);
    let output_debug = cargo_bin_cmd!("git-jump")
        .args(["--debug", "browse", "my-repo"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .output()
        .unwrap();

    assert!(output_normal.status.success());
    assert!(output_debug.status.success());
    assert_eq!(output_normal.stdout, output_debug.stdout);
}

#[test]
fn test_browse_without_config_shows_setup_hint() {
    let tmp = TempDir::new().unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse", "anything"])
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gj setup"));
}

#[test]
fn test_browse_help() {
    let output = cargo_bin_cmd!("git-jump")
        .args(["browse", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PATTERN") || stdout.contains("pattern"));
}

// -- current-dir-priority tests --

#[test]
fn test_browse_current_dir_detects_project() {
    let tmp = setup_browse_root();
    let project_dir = tmp.path().join("github.com/org/my-repo");

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://github.com/org/my-repo");
}

#[test]
fn test_browse_current_dir_from_subdirectory() {
    let tmp = setup_browse_root();
    let sub_dir = tmp.path().join("github.com/org/my-repo/src/lib");
    fs::create_dir_all(&sub_dir).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&sub_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://github.com/org/my-repo");
}

#[test]
fn test_browse_current_dir_subgroup_project() {
    let tmp = setup_browse_root();
    let project_dir = tmp.path().join("github.com/org/sub/deep-project");

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://github.com/org/sub/deep-project");
}

#[test]
fn test_browse_with_pattern_ignores_current_dir() {
    let tmp = setup_browse_root();
    // CWD is my-repo, but pattern asks for api-gateway
    let project_dir = tmp.path().join("github.com/org/my-repo");

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse", "api-gateway"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://github.com/org/api-gateway");
}

// -- Non-domain browse tests --

#[test]
fn test_browse_non_domain_with_web_url_template() {
    let tmp = setup_browse_root();

    let non_domain_project = tmp.path().join("personal/my-project");
    fs::create_dir_all(non_domain_project.join(".git")).unwrap();
    fs::write(
        non_domain_project.join(".git-jump.toml"),
        "web_url_template = \"https://github.com/user/my-project\"\n",
    )
    .unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&non_domain_project)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "https://github.com/user/my-project");
}

#[test]
fn test_browse_non_domain_no_web_url_template() {
    let tmp = setup_browse_root();

    let non_domain_project = tmp.path().join("personal/my-project");
    fs::create_dir_all(non_domain_project.join(".git")).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["browse"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&non_domain_project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no web_url_template configured"),
        "should report no web_url_template, got: {stderr}"
    );
}
