use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::TempDir;

#[test]
fn test_init_bash() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "bash"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gj()"));
    assert!(stdout.contains("eval"));
    assert!(
        stdout.contains("gjclone()"),
        "should contain gjclone function, got: {stdout}"
    );
}

#[test]
fn test_init_zsh() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "zsh"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gj()"));
}

#[test]
fn test_init_fish() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "fish"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("function gj"));
}

#[test]
fn test_init_unsupported_shell() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "powershell"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported shell"));
}

#[test]
fn test_init_auto_detect_from_shell_env() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["init"])
        .env("SHELL", "/usr/bin/zsh")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gj()"));
    assert!(stdout.contains("\\builtin local"));
}

#[test]
fn test_init_auto_detect_unsupported_shell_env() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["init"])
        .env("SHELL", "/usr/bin/tcsh")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported shell"));
    assert!(stderr.contains("gj init <bash|zsh|fish>"));
}

#[test]
fn test_init_no_placeholder_in_output() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "bash"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("__GIT_JUMP_CONFIG_DIR__"),
        "should not contain old placeholder, got: {stdout}"
    );
    assert!(
        !stdout.contains("logo.txt"),
        "should not reference logo.txt, got: {stdout}"
    );
}

#[test]
fn test_init_show_logo_flag_removed() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "--show-logo"]).output().unwrap();
    assert!(
        !output.status.success(),
        "--show-logo flag should be rejected"
    );
}

#[test]
fn test_init_bash_includes_completion() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "bash"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("complete"));
    assert!(stdout.contains("_gj_completions"));
}

#[test]
fn test_init_works_without_config() {
    let tmp = TempDir::new().unwrap();

    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd
        .args(["init", "bash"])
        .env("XDG_CONFIG_HOME", tmp.path().join(".config"))
        .env_remove("_GIT_JUMP_ROOT")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gj()"));
}

// Shell scripts should have logo_text environment switch detection

#[test]
fn test_init_bash_has_logo_text_check() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "bash"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("_GIT_JUMP_LOGO_TEXT"),
        "bash script should check _GIT_JUMP_LOGO_TEXT, got: {stdout}"
    );
    assert!(
        !stdout.contains("_GIT_JUMP_HAS_CONFIG"),
        "bash script should not reference old marker, got: {stdout}"
    );
}

#[test]
fn test_init_zsh_has_logo_text_check() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "zsh"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("_GIT_JUMP_LOGO_TEXT"),
        "zsh script should check _GIT_JUMP_LOGO_TEXT, got: {stdout}"
    );
}

#[test]
fn test_init_fish_has_logo_text_check() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["init", "fish"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("_GIT_JUMP_LOGO_TEXT"),
        "fish script should check _GIT_JUMP_LOGO_TEXT, got: {stdout}"
    );
}
