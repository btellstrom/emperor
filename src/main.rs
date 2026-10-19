use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use emperor::accept::{Selection, accept};
use emperor::check::{CheckOptions, check};
use emperor::doorstop::Tree;
use emperor::evidence::{self, Evidence};
use emperor::judge::{self, Fulfilment};
use emperor::lockfile::{self, Fingerprints};
use emperor::test_results::TestResults;
use emperor::{Error, locate, report};

/// `check` found problems.
const EXIT_PROBLEMS: u8 = 1;
/// emperor could not complete the command.
const EXIT_ERROR: u8 = 2;

#[derive(Parser)]
#[command(
    version,
    about = "Detect changed requirement justifications in a Doorstop tree"
)]
struct Cli {
    /// Root of the project containing the Doorstop tree and the lockfile.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    /// One line per problem and a summary.
    Human,
    /// A JSON document.
    Json,
    /// GitHub Actions error annotations.
    Github,
}

#[derive(Subcommand)]
enum Command {
    /// Fail if any justification changed since it was accepted.
    Check {
        /// Also fail for active requirements that have no justifications.
        #[arg(long)]
        require_justification: bool,
        /// JUnit XML test results; every test justifying a requirement must have passed.
        #[arg(long, value_name = "FILE")]
        test_results: Option<PathBuf>,
        /// How to report problems.
        #[arg(long, value_enum, default_value_t = Format::Human)]
        format: Format,
    },
    /// Ask the judge configured in emperor.toml whether requirements are fulfilled.
    Judge {
        /// Requirement IDs to judge.
        #[arg(required = true)]
        ids: Vec<String>,
        /// Fail when the judge finds a requirement unfulfilled.
        #[arg(long)]
        fail_on_unfulfilled: bool,
    },
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
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Check {
            require_justification,
            test_results,
            format,
        } => run_check(
            &cli.root,
            require_justification,
            test_results.as_deref(),
            format,
        ),
        Command::Accept { ids, all } => run_accept(&cli.root, ids, all),
        Command::Judge {
            ids,
            fail_on_unfulfilled,
        } => run_judge(&cli.root, &ids, fail_on_unfulfilled),
    };
    result.unwrap_or_else(|error| {
        eprintln!("error: {error}");
        ExitCode::from(EXIT_ERROR)
    })
}

fn read_test_results(path: Option<&Path>) -> emperor::Result<Option<TestResults>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let xml = std::fs::read_to_string(path).map_err(Error::io(path))?;
    TestResults::from_junit(&xml)
        .map(Some)
        .map_err(Error::parse(path))
}

/// The requirements, their justifications and the accepted fingerprints.
fn load_project(root: &Path) -> emperor::Result<(Tree, Evidence, Fingerprints)> {
    let tree = Tree::load(root)?;
    let evidence = evidence::gather(&tree)?;
    let locked = lockfile::read(root)?;
    Ok((tree, evidence, locked))
}

fn run_check(
    root: &Path,
    require_justification: bool,
    test_results: Option<&Path>,
    format: Format,
) -> emperor::Result<ExitCode> {
    let options = CheckOptions {
        require_justification,
        test_results: read_test_results(test_results)?,
    };
    let (tree, evidence, locked) = load_project(root)?;
    let problems = check(&tree, &evidence, &locked, &options)?;
    let report = match format {
        Format::Human => report::human(&problems),
        Format::Json => report::json(&problems),
        Format::Github => report::github(&problems, |justification| {
            locate::line_of(root, justification)
        }),
    };
    print!("{report}");
    Ok(if problems.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_PROBLEMS)
    })
}

fn run_accept(root: &Path, ids: Vec<String>, all: bool) -> emperor::Result<ExitCode> {
    let (tree, evidence, locked) = load_project(root)?;
    let selection = if all {
        Selection::All
    } else {
        Selection::Requirements(ids)
    };
    let accepted = accept(&tree, &evidence, &locked, &selection)?;
    lockfile::write(root, &accepted)?;
    Ok(ExitCode::SUCCESS)
}

fn run_judge(root: &Path, ids: &[String], fail_on_unfulfilled: bool) -> emperor::Result<ExitCode> {
    let config = judge::load_config(root)?;
    let tree = Tree::load(root)?;
    let evidence = evidence::gather(&tree)?;
    let mut any_unfulfilled = false;
    for uid in ids {
        let request = judge::request(&tree, &evidence, uid)?;
        let judgement = judge::judge(root, &config, uid, &request)?;
        println!("{judgement}");
        any_unfulfilled |= judgement.verdict.verdict == Fulfilment::Unfulfilled;
    }
    Ok(if any_unfulfilled && fail_on_unfulfilled {
        ExitCode::from(EXIT_PROBLEMS)
    } else {
        ExitCode::SUCCESS
    })
}
