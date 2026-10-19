use std::io;
use std::path::{Path, PathBuf};

use crate::justification::Justification;

/// Everything that stops emperor from completing a command.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("{}: {message}", path.display())]
    Parse { path: PathBuf, message: String },
    #[error("no Doorstop documents found under {}", .0.display())]
    NoDocuments(PathBuf),
    #[error("{prefix}: Markdown items are not supported")]
    MarkdownItems { prefix: String },
    #[error("{prefix}: unknown item format {format}")]
    UnknownItemFormat { prefix: String, format: String },
    #[error("{}: duplicate scenario name \"{name}\"", path.display())]
    DuplicateScenario { path: PathBuf, name: String },
    #[error("emperor.lock: unknown justification {0}")]
    UnknownLockfileKey(String),
    #[error("requirement {0} is defined more than once")]
    DuplicateRequirement(String),
    #[error("unknown requirement {0}")]
    UnknownRequirement(String),
    #[error("{requirement}: unknown justification type {kind}")]
    UnknownJustificationType { requirement: String, kind: String },
    #[error("{requirement}: a {kind} justification needs an `item`")]
    IncompleteJustification { requirement: String, kind: String },
    #[error("{requirement}: {justification} not found")]
    JustificationNotFound {
        requirement: String,
        justification: Justification,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Attaches the path an I/O error happened on: `.map_err(Error::io(path))`.
    pub fn io(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
        move |source| Error::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    /// Attaches the path of a file that could not be parsed: `.map_err(Error::parse(path))`.
    pub fn parse<E: std::fmt::Display>(path: &Path) -> impl FnOnce(E) -> Error + '_ {
        move |error| Error::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    }
}
