use assert_cmd::cargo::cargo_bin_cmd;

// -- Logo subcommand tests --

#[test]
fn test_logo_renders_figlet() {
    let output = cargo_bin_cmd!("git-jump")
        .args(["logo", "Test"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "should output FIGlet art");
    assert!(
        stdout.lines().count() > 1,
        "FIGlet output should be multi-line, got: {stdout}"
    );
}

#[test]
fn test_logo_no_args_empty_output() {
    let output = cargo_bin_cmd!("git-jump").args(["logo"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "no args should produce empty output");
}

#[test]
fn test_logo_empty_string_empty_output() {
    let output = cargo_bin_cmd!("git-jump")
        .args(["logo", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.is_empty(),
        "empty string should produce empty output"
    );
}

#[test]
fn test_logo_non_ascii_fallback() {
    let output = cargo_bin_cmd!("git-jump")
        .args(["logo", "\u{4f60}\u{597d}"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\u{4f60}\u{597d}"),
        "should output original text, got: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("non-ASCII"),
        "should warn about non-ASCII, got: {stderr}"
    );
}

#[test]
fn test_logo_help() {
    let output = cargo_bin_cmd!("git-jump")
        .args(["logo", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Render text as FIGlet"),
        "should show logo help, got: {stdout}"
    );
}
