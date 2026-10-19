//! Records the fingerprints of justifications after a person has reviewed them.

use std::collections::BTreeMap;

use crate::doorstop::{Item, Tree};
use crate::error::{Error, Result};
use crate::fingerprint;
use crate::justification;
use crate::lockfile::Fingerprints;

/// Which requirements to accept.
#[derive(Debug)]
pub enum Selection {
    /// Every active requirement; also forgets requirements that no longer exist.
    All,
    Requirements(Vec<String>),
}

/// The lockfile contents after accepting the selected requirements.
///
/// Either every selected requirement is accepted or, on error, nothing is.
pub fn accept(tree: &Tree, locked: &Fingerprints, selection: &Selection) -> Result<Fingerprints> {
    let mut accepted = locked.clone();
    for uid in selected_requirements(tree, locked, selection)? {
        let fingerprints = match tree.items.get(&uid) {
            Some(item) => fingerprints_of(tree, item)?,
            None => BTreeMap::new(),
        };
        if fingerprints.is_empty() {
            accepted.remove(&uid);
        } else {
            accepted.insert(uid, fingerprints);
        }
    }
    Ok(accepted)
}

/// The requirements to accept: existing ones, and removed ones that are still in the lockfile.
fn selected_requirements(
    tree: &Tree,
    locked: &Fingerprints,
    selection: &Selection,
) -> Result<Vec<String>> {
    match selection {
        Selection::All => {
            let active = tree.items.values().filter(|item| item.active);
            let removed = locked.keys().filter(|uid| !tree.items.contains_key(*uid));
            Ok(active
                .map(|item| item.uid.clone())
                .chain(removed.cloned())
                .collect())
        }
        Selection::Requirements(uids) => {
            let unknown = uids
                .iter()
                .find(|uid| !tree.items.contains_key(*uid) && !locked.contains_key(*uid));
            match unknown {
                Some(uid) => Err(Error::UnknownRequirement(uid.clone())),
                None => Ok(uids.clone()),
            }
        }
    }
}

fn fingerprints_of(tree: &Tree, item: &Item) -> Result<BTreeMap<String, String>> {
    let mut fingerprints = BTreeMap::new();
    for justification in justification::of_item(item)? {
        let Some(fingerprint) = fingerprint::of(&tree.root, &justification)? else {
            return Err(Error::JustificationNotFound {
                requirement: item.uid.clone(),
                justification,
            });
        };
        fingerprints.insert(justification.key(), fingerprint);
    }
    Ok(fingerprints)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::{Project, locked};

    fn requirements(uids: &[&str]) -> Selection {
        Selection::Requirements(uids.iter().map(|uid| uid.to_string()).collect())
    }

    #[test]
    fn records_the_current_fingerprints_of_a_requirement() {
        let project = Project::new().requirement("REQ001", &["a.rs"]);
        let accepted = accept(
            &project.tree(),
            &Fingerprints::new(),
            &requirements(&["REQ001"]),
        )
        .unwrap();
        assert_eq!(
            accepted,
            locked(&[("REQ001", "file:a.rs", &project.fingerprint_of("a.rs"))])
        );
    }

    #[test]
    fn leaves_other_requirements_untouched() {
        let project = Project::new()
            .requirement("REQ001", &["a.rs"])
            .requirement("REQ002", &["b.rs"]);
        let before = locked(&[("REQ002", "file:b.rs", "sha256:old")]);
        let accepted = accept(&project.tree(), &before, &requirements(&["REQ001"])).unwrap();
        assert_eq!(accepted["REQ002"], before["REQ002"]);
    }

    #[test]
    fn forgets_justifications_the_requirement_no_longer_names() {
        let project = Project::new().requirement("REQ001", &["a.rs"]);
        let before = locked(&[("REQ001", "file:gone.rs", "sha256:old")]);
        let accepted = accept(&project.tree(), &before, &requirements(&["REQ001"])).unwrap();
        assert!(!accepted["REQ001"].contains_key("file:gone.rs"));
    }

    #[test]
    fn a_requirement_without_justifications_has_no_entry() {
        let project = Project::new().requirement("REQ001", &[]);
        let accepted = accept(
            &project.tree(),
            &Fingerprints::new(),
            &requirements(&["REQ001"]),
        )
        .unwrap();
        assert_eq!(accepted, Fingerprints::new());
    }

    #[test]
    fn accepting_a_removed_requirement_forgets_it() {
        let before = locked(&[("REQ001", "file:a.rs", "sha256:old")]);
        let accepted = accept(&Project::new().tree(), &before, &requirements(&["REQ001"])).unwrap();
        assert_eq!(accepted, Fingerprints::new());
    }

    #[test]
    fn accepting_an_unknown_requirement_is_an_error() {
        let error = accept(
            &Project::new().tree(),
            &Fingerprints::new(),
            &requirements(&["REQ999"]),
        )
        .unwrap_err();
        assert!(matches!(error, Error::UnknownRequirement(uid) if uid == "REQ999"));
    }

    #[test]
    fn a_missing_justification_is_an_error() {
        let project = Project::new().requirement("REQ001", &["a.rs"]);
        fs::remove_file(project.path("a.rs")).unwrap();
        let error = accept(
            &project.tree(),
            &Fingerprints::new(),
            &requirements(&["REQ001"]),
        )
        .unwrap_err();
        assert!(matches!(error, Error::JustificationNotFound { .. }));
    }

    #[test]
    fn accepting_all_accepts_every_active_requirement_and_forgets_removed_ones() {
        let project = Project::new()
            .requirement("REQ001", &["a.rs"])
            .requirement("REQ002", &["b.rs"]);
        let before = locked(&[("REQ009", "file:gone.rs", "sha256:old")]);
        let accepted = accept(&project.tree(), &before, &Selection::All).unwrap();
        assert_eq!(accepted.keys().collect::<Vec<_>>(), ["REQ001", "REQ002"]);
    }

    #[test]
    fn accepting_all_keeps_inactive_requirements_as_they_were() {
        let project = Project::new()
            .requirement("REQ001", &["a.rs"])
            .inactive("REQ001");
        let before = locked(&[("REQ001", "file:a.rs", "sha256:old")]);
        let accepted = accept(&project.tree(), &before, &Selection::All).unwrap();
        assert_eq!(accepted, before);
    }
}
