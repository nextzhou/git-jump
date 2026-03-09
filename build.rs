fn main() {
    let version = std::process::Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().strip_prefix('v').unwrap_or(s.trim()).to_string())
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string())
        });

    println!("cargo::rustc-env=GJ_VERSION={version}");
}
