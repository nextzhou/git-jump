use assert_cmd::cargo::cargo_bin_cmd;

#[test]
fn test_help_flag() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.arg("--help").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn test_version_flag() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.arg("--version").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn test_no_args_shows_usage() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage") || stderr.contains("jump"));
}

#[test]
fn test_unknown_subcommand_errors() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.arg("notacommand").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_jump_subcommand_help() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["jump", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PATTERN"));
}

#[test]
fn test_setup_help() {
    let mut cmd = cargo_bin_cmd!("git-jump");
    let output = cmd.args(["setup", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("setup") || stdout.contains("Setup"));
}
