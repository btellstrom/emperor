pub mod doorstop;
pub mod lockfile;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use doorstop::{Item, Tree};
use lockfile::Fingerprints;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    /// The justification changed since it was last accepted.
    Changed { uid: String, key: String },
    /// The justification has never been accepted.
    Unaccepted { uid: String, key: String },
    /// The justification can no longer be found.
    Missing { uid: String, key: String },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::Changed { uid, key } => write!(f, "{uid}: {key} changed since it was accepted"),
            Problem::Unaccepted { uid, key } => write!(f, "{uid}: {key} has not been accepted"),
            Problem::Missing { uid, key } => write!(f, "{uid}: {key} not found"),
        }
    }
}

/// Compare the current justifications against the lockfile.
pub fn check(tree: &Tree, locked: &Fingerprints) -> Vec<Problem> {
    let mut problems = Vec::new();
    for item in tree.items.values().filter(|item| item.active) {
        let accepted = locked.get(&item.uid);
        for (key, current) in fingerprint_item(tree, item) {
            let uid = item.uid.clone();
            let Some(current) = current else {
                problems.push(Problem::Missing { uid, key });
                continue;
            };
            match accepted.and_then(|fingerprints| fingerprints.get(&key)) {
                None => problems.push(Problem::Unaccepted { uid, key }),
                Some(previous) if *previous != current => problems.push(Problem::Changed { uid, key }),
                Some(_) => {}
            }
        }
    }
    problems
}

/// Record the current fingerprints of the given items in the lockfile.
pub fn accept(tree: &Tree, locked: &mut Fingerprints, uids: &[String]) -> Result<()> {
    for uid in uids {
        let item = tree
            .items
            .get(uid)
            .ok_or_else(|| format!("unknown requirement {uid}"))?;
        let mut fingerprints = BTreeMap::new();
        for (key, current) in fingerprint_item(tree, item) {
            let current = current.ok_or_else(|| format!("{uid}: {key} not found"))?;
            fingerprints.insert(key, current);
        }
        locked.insert(uid.clone(), fingerprints);
    }
    Ok(())
}

/// Fingerprint every justification of an item, keyed by a stable description
/// of the justification. `None` means the justification could not be found.
fn fingerprint_item(tree: &Tree, item: &Item) -> Vec<(String, Option<String>)> {
    item.references
        .iter()
        .map(|reference| {
            let key = format!("{}:{}", reference.kind, reference.path);
            (key, fingerprint_file(&tree.root.join(&reference.path)))
        })
        .collect()
}

fn fingerprint_file(path: &Path) -> Option<String> {
    let contents = fs::read(path).ok()?;
    let hex: String = Sha256::digest(&contents)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Some(format!("sha256:{hex}"))
}
