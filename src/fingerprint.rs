//! Fingerprints identify the content of a justification.

use std::fs;
use std::io;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::justification::Justification;

/// The fingerprint of a justification, or `None` when it cannot be found.
pub fn of(root: &Path, justification: &Justification) -> Result<Option<String>> {
    match justification {
        Justification::File { path } => of_file(&root.join(path)),
    }
}

fn of_file(path: &Path) -> Result<Option<String>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(sha256(&contents))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io(path)(error)),
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:{hex}")
}
