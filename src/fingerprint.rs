//! Fingerprints identify the content of a justification.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::files::{read_if_present, read_source};
use crate::justification::Justification;
use crate::{gherkin_source, rust_source};

/// The fingerprint of a justification, or `None` when it cannot be found.
pub fn of(root: &Path, justification: &Justification) -> Result<Option<String>> {
    match justification {
        Justification::File { path } => {
            let contents = read_if_present(&root.join(path))?;
            Ok(contents.map(|contents| sha256(&contents)))
        }
        Justification::RustFn { path, item } => {
            let path = root.join(path);
            let Some(source) = read_source(&path)? else {
                return Ok(None);
            };
            rust_source::fingerprint_function(&source, item).map_err(Error::parse(&path))
        }
        Justification::Gherkin { path, scenario } => {
            let path = root.join(path);
            let Some(source) = read_source(&path)? else {
                return Ok(None);
            };
            let feature = gherkin_source::parse(&source).map_err(Error::parse(&path))?;
            Ok(gherkin_source::fingerprint_scenario(&feature, scenario))
        }
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:{hex}")
}
