//! Fingerprints identify the content of a justification.

use std::fs;
use std::io;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::justification::Justification;
use crate::rust_source;

/// The fingerprint of a justification, or `None` when it cannot be found.
pub fn of(root: &Path, justification: &Justification) -> Result<Option<String>> {
    match justification {
        Justification::File { path } => {
            let contents = read_if_present(&root.join(path))?;
            Ok(contents.map(|contents| sha256(&contents)))
        }
        Justification::RustFn { path, item } => {
            let path = root.join(path);
            let Some(contents) = read_if_present(&path)? else {
                return Ok(None);
            };
            let source = String::from_utf8_lossy(&contents);
            rust_source::fingerprint_function(&source, item).map_err(Error::parse(&path))
        }
    }
}

fn read_if_present(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
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
