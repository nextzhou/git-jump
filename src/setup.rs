use std::io::{self, Write};
use std::path::{Path, PathBuf};

use dialoguer::Confirm;

use crate::config;
use crate::debug::{self, DebugLog};
use crate::error::{Error, Result};
use crate::shell;

pub fn run(dbg: &mut DebugLog) -> Result<()> {
    let config_path = config::global_config_path()?;
    let domains_path = config::domains_file_path()?;

    if dbg.is_enabled() {
        dbg.log(&format!(
            "config file: {}",
            debug::abbreviate_path(&config_path)
        ));
        dbg.log(&format!(
            "domains file: {}",
            debug::abbreviate_path(&domains_path)
        ));
    }

    let existing = config::load_global_config()?;

    if config_path.exists() {
        eprintln!(
            "gj: config file already exists at {}",
            debug::abbreviate_path(&config_path)
        );
        let overwrite = prompt_confirm("Overwrite with new settings?", false)?;
        if !overwrite {
            return Err(Error::Cancelled);
        }
        eprintln!();
    }

    eprintln!("gj -- setup\n");

    let default_root = existing.root.clone().or_else(|| {
        std::env::current_dir()
            .ok()
            .map(|cwd| debug::abbreviate_path(&cwd))
    });

    eprintln!(
        "gj working directory: gjclone clones projects into this directory using the domain/group/project structure.\n"
    );

    let root = prompt_root(default_root.as_deref())?;

    eprintln!();
    eprintln!("  root = {root}");
    eprintln!();

    let confirmed = prompt_confirm("Save configuration?", true)?;
    if !confirmed {
        return Err(Error::Cancelled);
    }

    let config_dir = config_path
        .parent()
        .ok_or_else(|| Error::Config("invalid config path".into()))?;
    write_config(&config_path, &root, config_dir)?;
    eprintln!("\nSaved to {}", debug::abbreviate_path(&config_path));

    setup_shell_init()?;

    let config_display = debug::abbreviate_path(&config_path);
    eprintln!("\nSetup complete!");
    eprintln!("Config file: {config_display}");
    eprintln!(
        "The file contains a guide for hierarchical configuration -- review it to customize your setup."
    );

    Ok(())
}

fn setup_shell_init() -> Result<()> {
    let shell_name = match shell::detect_shell() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("\nCould not detect shell. Add manually to your rc file:");
            eprintln!("  eval \"$(git-jump init)\"");
            return Ok(());
        }
    };

    let rc_path = match shell_rc_path(&shell_name) {
        Some(p) => p,
        None => {
            eprintln!("\nAdd to your shell rc file:");
            eprintln!("  {}", init_line(&shell_name));
            return Ok(());
        }
    };

    let init = init_line(&shell_name);

    if rc_path.exists() {
        let content = std::fs::read_to_string(&rc_path).map_err(|e| Error::Io {
            source: e,
            path: Some(rc_path.clone()),
        })?;
        if content.contains("git-jump init") {
            eprintln!(
                "\nShell integration already configured in {}",
                rc_path.display()
            );
            return Ok(());
        }
    }

    let snippet = format!("\n# Added by git-jump setup\n{init}\n");

    eprintln!("\nThe following will be added to {}:", rc_path.display());
    for line in snippet.trim().lines() {
        eprintln!("  {line}");
    }
    eprintln!();

    let add = prompt_confirm("Proceed?", true)?;

    if !add {
        eprintln!("\nAdd manually later:");
        eprintln!("  {init}");
        return Ok(());
    }

    append_to_file(&rc_path, &snippet)?;
    eprintln!("Added to {}", rc_path.display());
    eprintln!(
        "Run `source {}` or restart your shell to activate.",
        rc_path.display()
    );

    Ok(())
}

fn shell_rc_path(shell_name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    match shell_name {
        "bash" => Some(home.join(".bashrc")),
        "zsh" => Some(home.join(".zshrc")),
        "fish" => Some(home.join(".config/fish/config.fish")),
        _ => None,
    }
}

fn init_line(shell_name: &str) -> String {
    match shell_name {
        "fish" => "command -q git-jump; and git-jump init fish | source".to_string(),
        _ => format!("command -v git-jump &>/dev/null && eval \"$(git-jump init {shell_name})\""),
    }
}

fn append_to_file(path: &Path, content: &str) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Io {
            source: e,
            path: Some(parent.to_path_buf()),
        })?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::Io {
            source: e,
            path: Some(path.to_path_buf()),
        })?;

    file.write_all(content.as_bytes()).map_err(|e| Error::Io {
        source: e,
        path: Some(path.to_path_buf()),
    })
}

/// Read a line of text from stdin with an optional default value.
///
/// Uses cooked-mode terminal input, which provides native readline behavior:
/// Ctrl-U (kill line), Ctrl-W (kill word), paste, backspace.
fn prompt_text(prompt: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(d) => eprint!("{prompt} [{d}]: "),
        None => eprint!("{prompt}: "),
    }
    io::stderr().flush()?;

    let mut line = String::new();
    let n = io::stdin().read_line(&mut line)?;
    if n == 0 {
        // EOF (e.g. Ctrl-D on empty line)
        return Err(Error::Cancelled);
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        Ok(default.unwrap_or("").to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn prompt_root(current: Option<&str>) -> Result<String> {
    loop {
        let raw = prompt_text("Project root directory", current)?;
        if raw.is_empty() {
            eprintln!("  error: cannot be empty");
            continue;
        }

        let expanded = config::expand_tilde(&raw);

        if expanded.is_dir() {
            return Ok(raw);
        }

        let create = prompt_confirm(
            &format!(
                "Directory '{}' does not exist. Create it?",
                expanded.display()
            ),
            true,
        )?;

        if create {
            std::fs::create_dir_all(&expanded).map_err(|e| Error::Io {
                source: e,
                path: Some(expanded),
            })?;
            return Ok(raw);
        }
    }
}

fn prompt_confirm(message: &str, default: bool) -> Result<bool> {
    Confirm::new()
        .with_prompt(message)
        .default(default)
        .interact()
        .map_err(|e| Error::Config(format!("prompt error: {e}")))
}

pub fn write_config(path: &Path, root: &str, config_dir: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Io {
            source: e,
            path: Some(parent.to_path_buf()),
        })?;
    }

    let content = format_config(root, config_dir);

    std::fs::write(path, content).map_err(|e| Error::Io {
        source: e,
        path: Some(path.to_path_buf()),
    })
}

fn format_config(root: &str, config_dir: &Path) -> String {
    let config_display = config_dir.display();
    let root_escaped = escape_toml_string(root);

    format!(
        "\
# gj (git-jump) -- global configuration
#
# Location: {config_display}/config.toml
# Generated by `gj setup`. Feel free to edit.

# Project root directory.
# All projects are organized as root/<domain>/<group>/<project>.
# Can also be set via the $_GIT_JUMP_ROOT environment variable (takes precedence).
root = \"{root_escaped}\"

# Browser command template (used by `git-jump browse`).
# Use {{url}} as a placeholder for the target URL.
# Omit to use the system default browser.
# browser = \"\"

# ASCII art logo text.
# When jumping to a project with a different logo_text, gj renders this text with FIGlet.
logo_text = \"Git Jump\"

# -----------------------------------------------------------------
# Hierarchical configuration
# -----------------------------------------------------------------
#
# Place a `.git-jump.toml` at any directory level under the project root ({root_escaped})
# to customize behavior for all projects beneath it:
#
#   {root_escaped}/<domain>/.git-jump.toml                      -- domain level
#   {root_escaped}/<domain>/<group>/.git-jump.toml              -- group level
#   {root_escaped}/<domain>/<group>/<subgroup>/.git-jump.toml   -- subgroup level
#   {root_escaped}/<domain>/.../<project>/.git-jump.toml        -- project level
#
# Example `.git-jump.toml`:
#
#   # Browse URL template -- used by `git-jump browse`.
#   # Available variables: {{domain}}, {{groups}}, {{project}}, {{branch}}, {{path}}
#   # {{branch}} = current local Git branch name (git command runs only when used)
#   # {{path}} = subdirectory path relative to git root (empty if at root)
#   #
#   # GitLab:  web_url_template = \"https://{{domain}}/{{groups}}/{{project}}/-/tree/{{branch}}/{{path}}\"
#   # GitHub:  web_url_template = \"https://{{domain}}/{{groups}}/{{project}}/tree/{{branch}}/{{path}}\"
#   # Project home only: web_url_template = \"https://{{domain}}:8443/{{groups}}/{{project}}\"
#
#   # Git configuration -- applied each time you jump into a project under this path.
#   # Same key: child overrides parent. Different keys: merged.
#   # Note: keys with dots must be quoted.
#   [git_config]
#   \"user.name\" = \"Your Name\"
#   \"user.email\" = \"you@company.com\"
#
#   # Environment variables -- set each time you jump.
#   # Same key: child overrides parent. Different keys: merged.
#   [env]
#   GOPATH = \"/home/you/go\"
#   RUST_LOG = \"debug\"
#
#   # Hooks -- commands executed each time you jump.
#   # Append mode: all levels execute from parent to child.
#   [hooks]
#   on_enter = [\"echo 'Welcome'\", \"make check-env\"]
"
    )
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config_dir() -> PathBuf {
        PathBuf::from("/home/user/.config/git-jump")
    }

    #[test]
    fn format_config_basic() {
        let config_dir = test_config_dir();
        let result = format_config("/home/user/code", &config_dir);
        assert!(result.contains("root = \"/home/user/code\""));
    }

    #[test]
    fn format_config_escapes_special_chars() {
        let config_dir = test_config_dir();
        let result = format_config("C:\\Users\\code", &config_dir);
        assert!(result.contains("root = \"C:\\\\Users\\\\code\""));
    }

    #[test]
    fn write_and_read_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join("git-jump");
        let config_path = config_dir.join("config.toml");

        write_config(&config_path, "~/code", &config_dir).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: config::GlobalConfig = toml::from_str(&content).unwrap();

        assert_eq!(parsed.root.as_deref(), Some("~/code"));
    }

    #[test]
    fn format_config_contains_header() {
        let config_dir = test_config_dir();
        let result = format_config("~/code", &config_dir);
        assert!(
            result.contains("# gj (git-jump) -- global configuration"),
            "should contain header title"
        );
        assert!(
            result.contains(&format!("# Location: {}/config.toml", config_dir.display())),
            "should contain actual config path in header"
        );
        assert!(
            result.contains("# Generated by `gj setup`. Feel free to edit."),
            "should mention generated by setup"
        );
    }

    #[test]
    fn format_config_contains_per_dir_guide() {
        let config_dir = test_config_dir();
        let result = format_config("~/code", &config_dir);
        assert!(
            result.contains("# Hierarchical configuration"),
            "should contain per-dir config section header"
        );
        assert!(
            result.contains("~/code/<domain>/<group>/<subgroup>/.git-jump.toml"),
            "should use actual root path in subgroup level"
        );
        assert!(
            result.contains("~/code/<domain>/.git-jump.toml"),
            "should use actual root path in domain level"
        );
        assert!(
            !result.contains("<root>"),
            "should not contain <root> placeholder"
        );
        assert!(
            result.contains("[git_config]"),
            "should show git_config example"
        );
        assert!(result.contains("[env]"), "should show env example");
        assert!(result.contains("[hooks]"), "should show hooks example");
        assert!(
            result.contains("web_url_template"),
            "should mention web_url_template in per-dir guide"
        );
        assert!(
            result.contains("{branch}"),
            "should mention {{branch}} placeholder"
        );
        assert!(
            result.contains("{path}"),
            "should mention {{path}} placeholder"
        );
    }

    #[test]
    fn format_config_uses_config_dir() {
        let custom_dir = PathBuf::from("/custom/xdg/git-jump");
        let result = format_config("~/code", &custom_dir);
        assert!(
            result.contains("/custom/xdg/git-jump/config.toml"),
            "header should use custom config_dir path"
        );
        assert!(
            !result.contains("~/.config/git-jump"),
            "should not contain hardcoded default path"
        );
    }

    #[test]
    fn format_config_contains_logo_text() {
        let config_dir = test_config_dir();
        let result = format_config("~/code", &config_dir);
        assert!(
            result.contains("logo_text = \"Git Jump\""),
            "should contain logo_text field, got: {result}"
        );
        assert!(
            result.contains("FIGlet"),
            "should mention FIGlet in logo_text comment"
        );
        assert!(
            !result.contains("logo.txt"),
            "should not reference old logo.txt file"
        );
    }

    #[test]
    fn escape_toml_string_no_special() {
        assert_eq!(escape_toml_string("hello"), "hello");
    }

    #[test]
    fn escape_toml_string_with_backslash() {
        assert_eq!(escape_toml_string("a\\b"), "a\\\\b");
    }

    #[test]
    fn escape_toml_string_with_quote() {
        assert_eq!(escape_toml_string("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn init_line_bash() {
        assert_eq!(
            init_line("bash"),
            "command -v git-jump &>/dev/null && eval \"$(git-jump init bash)\""
        );
    }

    #[test]
    fn init_line_zsh() {
        assert_eq!(
            init_line("zsh"),
            "command -v git-jump &>/dev/null && eval \"$(git-jump init zsh)\""
        );
    }

    #[test]
    fn init_line_fish() {
        assert_eq!(
            init_line("fish"),
            "command -q git-jump; and git-jump init fish | source"
        );
    }

    #[test]
    fn shell_rc_path_known_shells() {
        assert!(shell_rc_path("bash").unwrap().ends_with(".bashrc"));
        assert!(shell_rc_path("zsh").unwrap().ends_with(".zshrc"));
        assert!(
            shell_rc_path("fish")
                .unwrap()
                .ends_with(".config/fish/config.fish")
        );
    }

    #[test]
    fn shell_rc_path_unknown() {
        assert!(shell_rc_path("powershell").is_none());
    }

    #[test]
    fn append_to_file_creates_and_appends() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test_rc");

        append_to_file(&path, "line1\n").unwrap();
        append_to_file(&path, "line2\n").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "line1\nline2\n");
    }
}
