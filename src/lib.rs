pub mod check;
pub mod doorstop;
pub mod fingerprint;
pub mod justification;
pub mod lockfile;

use std::collections::BTreeMap;

use doorstop::Tree;
use lockfile::Fingerprints;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Record the current fingerprints of the given items in the lockfile.
pub fn accept(tree: &Tree, locked: &mut Fingerprints, uids: &[String]) -> Result<()> {
    for uid in uids {
        let item = tree
            .items
            .get(uid)
            .ok_or_else(|| format!("unknown requirement {uid}"))?;
        let mut fingerprints = BTreeMap::new();
        for justification in justification::of_item(item) {
            let current = fingerprint::of(&tree.root, &justification)?
                .ok_or_else(|| format!("{uid}: {justification} not found"))?;
            fingerprints.insert(justification.to_string(), current);
        }
        locked.insert(uid.clone(), fingerprints);
    }
    Ok(())
}
