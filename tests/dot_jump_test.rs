use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::TempDir;

// -- Current Project Awareness tests (gj .) --

fn setup_dot_jump_root() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let config_dir = root.join(".config/git-jump");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("root = \"{}\"\n", root.display()),
    )
    .unwrap();
    fs::write(config_dir.join("domains"), "example.com\n").unwrap();

    // Domain project with config
    let domain_dir = root.join("example.com");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(
        domain_dir.join(".git-jump.toml"),
        "[git_config]\n\"user.name\" = \"Domain User\"\n\n[hooks]\non_enter = [\"echo domain-hook\"]\n",
    )
    .unwrap();

    let project_dir = domain_dir.join("team/my-repo");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::create_dir_all(project_dir.join("src/lib")).unwrap();
    fs::write(
        project_dir.join(".git-jump.toml"),
        "[hooks]\non_enter = [\"echo project-hook\"]\n",
    )
    .unwrap();

    // Another domain project (no config)
    let bare_project = domain_dir.join("team/bare-repo");
    fs::create_dir_all(bare_project.join(".git")).unwrap();

    tmp
}

// AC-1: gj . basic jump from subdirectory
#[test]
fn test_dot_jump_from_subdirectory() {
    let tmp = setup_dot_jump_root();
    let sub_dir = tmp.path().join("example.com/team/my-repo/src/lib");

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&sub_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("builtin cd"),
        "should contain cd command, got: {stdout}"
    );
    assert!(
        stdout.contains("my-repo"),
        "should cd to project root, got: {stdout}"
    );
    assert!(
        !stdout.contains("src/lib"),
        "should not contain subdirectory, got: {stdout}"
    );
}

// AC-2: gj . domain project config applied
#[test]
fn test_dot_jump_domain_project_with_config() {
    let tmp = setup_dot_jump_root();
    let sub_dir = tmp.path().join("example.com/team/my-repo/src");
    fs::create_dir_all(&sub_dir).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&sub_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("builtin cd"), "should contain cd");
    assert!(
        stdout.contains("git config"),
        "should apply git_config, got: {stdout}"
    );
    assert!(
        stdout.contains("user.name"),
        "should set user.name, got: {stdout}"
    );
    assert!(
        stdout.contains("echo domain-hook"),
        "should include domain hook, got: {stdout}"
    );
    assert!(
        stdout.contains("echo project-hook"),
        "should include project hook, got: {stdout}"
    );
}

// AC-3: gj . non-domain project config scan
#[test]
fn test_dot_jump_non_domain_config_scan() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // No global config (non-domain scenario)
    let parent_dir = root.join("a");
    fs::create_dir_all(&parent_dir).unwrap();
    fs::write(
        parent_dir.join(".git-jump.toml"),
        "[env]\nFOO = \"parent\"\n",
    )
    .unwrap();

    let project_dir = parent_dir.join("project");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::write(
        project_dir.join(".git-jump.toml"),
        "[env]\nBAR = \"project\"\n",
    )
    .unwrap();

    let sub_dir = project_dir.join("src");
    fs::create_dir_all(&sub_dir).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .current_dir(&sub_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export FOO='parent'"),
        "should load parent config, got: {stdout}"
    );
    assert!(
        stdout.contains("export BAR='project'"),
        "should load project config, got: {stdout}"
    );
}

// AC-6: gj . pure cd no config
#[test]
fn test_dot_jump_pure_cd_no_config() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let project_dir = root.join("project");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    let sub_dir = project_dir.join("src");
    fs::create_dir_all(&sub_dir).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .current_dir(&sub_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("builtin cd"), "should contain cd");
    assert!(
        stdout.contains("export _GIT_JUMP_LOGO_TEXT=''"),
        "should always have logo_text export, got: {stdout}"
    );
    assert!(
        !stdout.contains("git config"),
        "should not have git config, got: {stdout}"
    );
    assert!(
        !stdout.contains("_GIT_JUMP_HAS_CONFIG"),
        "should not have old config marker, got: {stdout}"
    );
}

// AC-7: gj . always outputs _GIT_JUMP_LOGO_TEXT
#[test]
fn test_dot_jump_outputs_logo_text() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let project_dir = root.join("project");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::write(project_dir.join(".git-jump.toml"), "[env]\nFOO = \"bar\"\n").unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export _GIT_JUMP_LOGO_TEXT="),
        "should have logo_text export, got: {stdout}"
    );
    assert!(
        stdout.contains("export FOO='bar'"),
        "should have env export, got: {stdout}"
    );
}

// AC-8: gj . not in git repo
#[test]
fn test_dot_jump_not_in_git_repo() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("not-a-repo");
    fs::create_dir_all(&dir).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .current_dir(&dir)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not in a git repository"),
        "should report not in git repo, got: {stderr}"
    );
}

// AC-9: gj . with .git as file (submodule/worktree)
#[test]
fn test_dot_jump_git_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let project_dir = root.join("project");
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join(".git"), "gitdir: /fake/path\n").unwrap();

    let sub_dir = project_dir.join("src");
    fs::create_dir_all(&sub_dir).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .current_dir(&sub_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("project'"),
        "should cd to git root, got: {stdout}"
    );
}

// AC-10: gj . debug output
#[test]
fn test_dot_jump_debug_output() {
    let tmp = setup_dot_jump_root();
    let sub_dir = tmp.path().join("example.com/team/my-repo/src");
    fs::create_dir_all(&sub_dir).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["--debug", "jump", "."])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&sub_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("dot jump"),
        "should show dot jump, got: {stderr}"
    );
    assert!(
        stderr.contains("git root"),
        "should show git root, got: {stderr}"
    );
    assert!(
        stderr.contains("project class"),
        "should show project class, got: {stderr}"
    );
    assert!(
        stderr.contains("shell commands"),
        "should show shell commands, got: {stderr}"
    );
}

// AC-14: gj . at project root
#[test]
fn test_dot_jump_at_project_root() {
    let tmp = setup_dot_jump_root();
    let project_dir = tmp.path().join("example.com/team/my-repo");

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("builtin cd"), "should contain cd");
    assert!(
        stdout.contains("git config"),
        "should apply config, got: {stdout}"
    );
}

// AC-15: non-domain hooks parent-to-child order
#[test]
fn test_dot_jump_non_domain_hooks_order() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let parent_dir = root.join("a");
    fs::create_dir_all(&parent_dir).unwrap();
    fs::write(
        parent_dir.join(".git-jump.toml"),
        "[hooks]\non_enter = [\"echo parent\"]\n",
    )
    .unwrap();

    let project_dir = parent_dir.join("project");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::write(
        project_dir.join(".git-jump.toml"),
        "[hooks]\non_enter = [\"echo project\"]\n",
    )
    .unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent_pos = stdout.find("echo parent");
    let project_pos = stdout.find("echo project");
    assert!(parent_pos.is_some(), "should have parent hook");
    assert!(project_pos.is_some(), "should have project hook");
    assert!(
        parent_pos.unwrap() < project_pos.unwrap(),
        "parent hook should come before project hook"
    );
}

// AC-16: non-domain env override
#[test]
fn test_dot_jump_non_domain_env_override() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let parent_dir = root.join("a");
    fs::create_dir_all(&parent_dir).unwrap();
    fs::write(
        parent_dir.join(".git-jump.toml"),
        "[env]\nFOO = \"parent\"\n",
    )
    .unwrap();

    let project_dir = parent_dir.join("project");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::write(
        project_dir.join(".git-jump.toml"),
        "[env]\nFOO = \"project\"\n",
    )
    .unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export FOO='project'"),
        "child should override parent, got: {stdout}"
    );
    assert!(
        !stdout.contains("FOO='parent'"),
        "parent value should be overridden, got: {stdout}"
    );
}

// AC-22: empty config file does not count as "has config"
#[test]
fn test_dot_jump_empty_config_no_marker() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let project_dir = root.join("project");
    fs::create_dir_all(project_dir.join(".git")).unwrap();
    fs::write(project_dir.join(".git-jump.toml"), "").unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "."])
        .env("XDG_CONFIG_HOME", root.join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export _GIT_JUMP_LOGO_TEXT=''"),
        "should always have logo_text export, got: {stdout}"
    );
}

// -- Current project pinning tests --

// AC-11: gj no-args current project pinned to top (non-tty picks first = pinned)
#[test]
fn test_no_args_current_project_pinned() {
    let tmp = setup_dot_jump_root();
    // CWD inside bare-repo, which comes after my-repo alphabetically
    let project_dir = tmp.path().join("example.com/team/bare-repo");

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("bare-repo"),
        "pinned project should be selected (first in non-tty), got: {stdout}"
    );
}

// AC-13: gj <pattern> does not pin
#[test]
fn test_pattern_no_pin() {
    let tmp = setup_dot_jump_root();
    // CWD inside bare-repo, but pattern matches my-repo
    let project_dir = tmp.path().join("example.com/team/bare-repo");

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump", "my-repo"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&project_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("my-repo"),
        "should match my-repo, not pin bare-repo, got: {stdout}"
    );
}

// AC-12b: gj no-args not in git repo
#[test]
fn test_no_args_not_in_git_repo() {
    let tmp = setup_dot_jump_root();
    let non_git_dir = tmp.path().join("some-random-dir");
    fs::create_dir_all(&non_git_dir).unwrap();

    let output = cargo_bin_cmd!("git-jump")
        .args(["jump"])
        .env("_GIT_JUMP_ROOT", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .current_dir(&non_git_dir)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "should not error when not in git repo"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("builtin cd"),
        "should still select a project"
    );
}
