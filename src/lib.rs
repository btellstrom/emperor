pub mod check;
pub mod doorstop;
pub mod error;
pub mod fingerprint;
pub mod justification;
pub mod lockfile;

use std::collections::BTreeMap;

use doorstop::Tree;
pub use error::{Error, Result};
use lockfile::Fingerprints;

/// Record the current fingerprints of the given items in the lockfile.
pub fn accept(tree: &Tree, locked: &mut Fingerprints, uids: &[String]) -> Result<()> {
    for uid in uids {
        let item = tree
            .items
            .get(uid)
            .ok_or_else(|| Error::UnknownRequirement(uid.clone()))?;
        let mut fingerprints = BTreeMap::new();
        for justification in justification::of_item(item) {
            let Some(current) = fingerprint::of(&tree.root, &justification)? else {
                return Err(Error::JustificationNotFound {
                    requirement: uid.clone(),
                    justification,
                });
            };
            fingerprints.insert(justification.to_string(), current);
        }
        locked.insert(uid.clone(), fingerprints);
    }
    Ok(())
}
