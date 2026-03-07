use std::fmt;
use std::path::PathBuf;

/// All error types for gj.
#[derive(Debug)]
pub enum Error {
    /// Configuration file is missing or malformed.
    Config(String),
    /// Filesystem I/O error with optional path context.
    Io {
        source: std::io::Error,
        path: Option<PathBuf>,
    },
    /// No project matched the given pattern.
    NoMatch { pattern: String },
    /// The root directory is not configured or does not exist.
    RootNotFound { path: PathBuf },
    /// User cancelled the selection (Esc). Exit code 1.
    Cancelled,
    /// User interrupted with Ctrl-C. Exit code 130 (128 + SIGINT).
    Interrupted,
    /// TOML parsing error.
    TomlParse(toml::de::Error),
    /// Global config file missing; user should run `gj setup`.
    SetupRequired,
    /// Current directory is not inside a git repository.
    NotInGitRepo,
    /// No web_url_template configured for this project.
    NoWebUrlTemplate,
    /// Failed to detect current branch via git.
    BranchDetectFailed { project: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Config(msg) => write!(f, "config error: {msg}"),
            Error::Io {
                source,
                path: Some(p),
            } => {
                write!(f, "IO error at {}: {source}", p.display())
            }
            Error::Io { source, path: None } => write!(f, "IO error: {source}"),
            Error::NoMatch { pattern } => write!(f, "no project matching '{pattern}'"),
            Error::RootNotFound { path } => {
                write!(f, "project root not found: {}", path.display())
            }
            Error::Cancelled => write!(f, "selection cancelled"),
            Error::Interrupted => write!(f, "interrupted"),
            Error::TomlParse(err) => write!(f, "config parse error: {err}"),
            Error::SetupRequired => {
                write!(f, "not configured yet. Run `gj setup` to get started")
            }
            Error::NotInGitRepo => write!(f, "not in a git repository"),
            Error::NoWebUrlTemplate => {
                write!(f, "no web_url_template configured for this project")
            }
            Error::BranchDetectFailed { project } => {
                write!(f, "failed to detect current branch for {project}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            Error::TomlParse(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io {
            source: err,
            path: None,
        }
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Self {
        Error::TomlParse(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
