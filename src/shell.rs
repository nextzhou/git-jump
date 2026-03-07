use crate::error::{Error, Result};

const BASH_SCRIPT: &str = include_str!("../shell/gj.bash");
const ZSH_SCRIPT: &str = include_str!("../shell/gj.zsh");
const FISH_SCRIPT: &str = include_str!("../shell/gj.fish");

pub fn init_script(shell: &str) -> Result<String> {
    let script: &str = match shell {
        "bash" => BASH_SCRIPT,
        "zsh" => ZSH_SCRIPT,
        "fish" => FISH_SCRIPT,
        _ => {
            return Err(Error::Config(format!(
                "unsupported shell: {shell} (expected bash, zsh, or fish)"
            )));
        }
    };
    Ok(script.to_string())
}

pub fn detect_shell() -> Result<String> {
    let shell_path = std::env::var("SHELL").map_err(|_| {
        Error::Config(
            "cannot detect shell: $SHELL is not set. Please specify: gj init <bash|zsh|fish>"
                .into(),
        )
    })?;

    let name = std::path::Path::new(&shell_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    match name {
        "bash" | "zsh" | "fish" => Ok(name.to_string()),
        _ => Err(Error::Config(format!(
            "unsupported shell '{name}' (from $SHELL={shell_path}). \
             Please specify: gj init <bash|zsh|fish>"
        ))),
    }
}
