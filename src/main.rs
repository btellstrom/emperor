use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use emperor::doorstop::Tree;
use emperor::lockfile;

#[derive(Parser)]
#[command(version, about = "Detect changed requirement justifications in a Doorstop tree")]
struct Cli {
    /// Root of the project containing the Doorstop tree and the lockfile.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fail if any justification changed since it was accepted.
    Check,
    /// Record the current justifications of requirements as reviewed.
    Accept {
        /// Requirement IDs to accept.
        #[arg(required_unless_present = "all")]
        ids: Vec<String>,
        /// Accept every requirement.
        #[arg(long, conflicts_with = "ids")]
        all: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> emperor::Result<ExitCode> {
    let tree = Tree::load(&cli.root)?;
    let mut locked = lockfile::read(&cli.root)?;

    match cli.command {
        Command::Check => {
            let problems = emperor::check(&tree, &locked);
            for problem in &problems {
                println!("{problem}");
            }
            Ok(if problems.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        Command::Accept { ids, all } => {
            let ids = if all { tree.items.keys().cloned().collect() } else { ids };
            emperor::accept(&tree, &mut locked, &ids)?;
            lockfile::write(&cli.root, &locked)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
