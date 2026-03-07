mod browse;
mod clone;
mod completions;
mod config;
mod debug;
mod error;
mod filter;
mod jump;
mod project;
mod resolve;
mod score;
mod select;
mod setup;
mod shell;

use std::process;
use std::time::Instant;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "git-jump",
    about = "Quickly jump between Git projects",
    version,
    subcommand_required = true
)]
struct Cli {
    /// Print debug information to stderr
    #[arg(long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a matching project's web page in the browser
    Browse {
        /// Filter tokens (case-insensitive substring match against display text)
        pattern: Vec<String>,
    },
    /// Clone a Git repository into the organized directory structure
    Clone {
        /// Repository URL or group/project shorthand
        repo: String,
    },
    /// Output shell integration script
    Init {
        /// Shell type (bash, zsh, fish)
        shell: Option<String>,
    },
    /// Interactive first-time configuration
    Setup,
    /// Generate tab completions for the given shell and partial input
    Completions {
        /// Shell type (bash, zsh, fish)
        shell: String,
        /// Partial input to complete
        partial: Option<String>,
    },
    /// Jump to a matching project
    Jump {
        /// Filter tokens (case-insensitive substring match against display text)
        pattern: Vec<String>,
    },
    /// Render text as FIGlet ASCII art
    Logo {
        /// Text to render
        text: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let mut dbg = debug::DebugLog::new(cli.debug);
    let start = Instant::now();

    let result = run(cli.command, &mut dbg);

    dbg.log(&format!(
        "total: {:.1}ms",
        start.elapsed().as_secs_f64() * 1000.0
    ));
    dbg.flush();

    if let Err(e) = result {
        let code = match &e {
            error::Error::Cancelled => 1,
            error::Error::Interrupted => 130,
            _ => {
                eprintln!("gj: {e}");
                1
            }
        };
        process::exit(code);
    }
}

fn run(command: Commands, dbg: &mut debug::DebugLog) -> error::Result<()> {
    match command {
        Commands::Browse { pattern } => {
            require_setup()?;
            let global = config::load_global_config()?;
            browse::run(&pattern, &global, dbg)
        }
        Commands::Setup => setup::run(dbg),
        Commands::Init { shell } => {
            let (shell_name, shell_source) = match shell {
                Some(s) => (s, "(from argument)"),
                None => (shell::detect_shell()?, "(from $SHELL)"),
            };
            dbg.log(&format!("shell: {shell_name} {shell_source}"));

            let script = shell::init_script(&shell_name)?;
            print!("{script}");
            Ok(())
        }
        Commands::Clone { repo } => {
            require_setup()?;
            let global = config::load_global_config()?;
            let output = clone::run(&repo, &global, dbg)?;
            println!("{output}");
            Ok(())
        }
        Commands::Completions { shell: _, partial } => {
            require_setup()?;
            let global = config::load_global_config()?;
            let output = completions::run(partial.as_deref(), &global, dbg)?;
            if !output.is_empty() {
                print!("{output}");
            }
            Ok(())
        }
        Commands::Jump { pattern } => {
            let is_dot_jump = pattern.len() == 1 && pattern[0] == ".";
            let global = if is_dot_jump {
                config::load_global_config().ok()
            } else {
                require_setup()?;
                Some(config::load_global_config()?)
            };
            let output = jump::run(&pattern, global.as_ref(), dbg)?;
            print!("{output}");
            Ok(())
        }
        Commands::Logo { text } => {
            let text = text.as_deref().unwrap_or("");
            if text.is_empty() {
                return Ok(());
            }
            if text.bytes().any(|b| b > 0x7F) {
                println!("{text}");
                eprintln!(
                    "warning: logo_text contains non-ASCII characters, FIGlet rendering skipped"
                );
                return Ok(());
            }
            let font = figlet_rs::FIGfont::standard()
                .map_err(|e| error::Error::Config(format!("FIGlet font error: {e}")))?;
            if let Some(figure) = font.convert(text) {
                print!("{figure}");
            }
            Ok(())
        }
    }
}

fn require_setup() -> error::Result<()> {
    if !config::config_file_exists() {
        return Err(error::Error::SetupRequired);
    }
    Ok(())
}
