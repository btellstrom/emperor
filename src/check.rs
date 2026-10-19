//! Compares the current justifications with the fingerprints accepted in the lockfile.

use std::fmt;

use crate::doorstop::Tree;
use crate::error::Result;
use crate::fingerprint;
use crate::justification::{self, Justification};
use crate::lockfile::Fingerprints;

#[derive(Debug, Default)]
pub struct CheckOptions {
    /// Report active requirements that have no justifications at all.
    pub require_justification: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    /// The justification changed since it was last accepted.
    Changed {
        requirement: String,
        justification: Justification,
    },
    /// The justification has never been accepted.
    Unaccepted {
        requirement: String,
        justification: Justification,
    },
    /// The justification can no longer be found.
    Missing {
        requirement: String,
        justification: Justification,
    },
    /// The requirement has no justifications, and justifications are required.
    Unjustified { requirement: String },
    /// The lockfile has fingerprints for a requirement that no longer exists.
    RequirementRemoved { requirement: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Changed {
                requirement,
                justification,
            } => {
                write!(
                    f,
                    "{requirement}: {justification} changed since it was accepted"
                )
            }
            Problem::Unaccepted {
                requirement,
                justification,
            } => {
                write!(f, "{requirement}: {justification} has not been accepted")
            }
            Problem::Missing {
                requirement,
                justification,
            } => {
                write!(f, "{requirement}: {justification} not found")
            }
            Problem::Unjustified { requirement } => {
                write!(f, "{requirement}: has no justifications")
            }
            Problem::RequirementRemoved { requirement } => {
                write!(f, "{requirement}: requirement no longer exists")
            }
        }
    }
}

/// Every problem in the tree, in requirement order.
pub fn check(tree: &Tree, locked: &Fingerprints, options: &CheckOptions) -> Result<Vec<Problem>> {
    let mut problems = Vec::new();
    for item in tree.items.values().filter(|item| item.active) {
        let justifications = justification::of_item(item);
        if justifications.is_empty() && options.require_justification {
            problems.push(Problem::Unjustified {
                requirement: item.uid.clone(),
            });
        }
        for justification in justifications {
            problems.extend(check_justification(tree, locked, &item.uid, justification)?);
        }
    }
    problems.extend(removed_requirements(tree, locked));
    Ok(problems)
}

fn check_justification(
    tree: &Tree,
    locked: &Fingerprints,
    uid: &str,
    justification: Justification,
) -> Result<Option<Problem>> {
    let requirement = uid.to_string();
    let Some(current) = fingerprint::of(&tree.root, &justification)? else {
        return Ok(Some(Problem::Missing {
            requirement,
            justification,
        }));
    };
    let accepted = locked
        .get(uid)
        .and_then(|fingerprints| fingerprints.get(&justification.to_string()));
    Ok(match accepted {
        None => Some(Problem::Unaccepted {
            requirement,
            justification,
        }),
        Some(accepted) if *accepted != current => Some(Problem::Changed {
            requirement,
            justification,
        }),
        Some(_) => None,
    })
}

fn removed_requirements<'a>(
    tree: &'a Tree,
    locked: &'a Fingerprints,
) -> impl Iterator<Item = Problem> + 'a {
    locked
        .keys()
        .filter(|uid| !tree.items.contains_key(*uid))
        .map(|uid| Problem::RequirementRemoved {
            requirement: uid.clone(),
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::doorstop::{Item, Reference};

    struct Fixture {
        project: TempDir,
        items: Vec<Item>,
        locked: Fingerprints,
    }

    impl Fixture {
        fn new() -> Fixture {
            Fixture {
                project: TempDir::new().unwrap(),
                items: Vec::new(),
                locked: Fingerprints::new(),
            }
        }

        fn requirement(mut self, uid: &str, files: &[&str]) -> Fixture {
            for file in files {
                fs::write(
                    self.project.path().join(file),
                    format!("contents of {file}"),
                )
                .unwrap();
            }
            self.items.push(Item {
                uid: uid.to_string(),
                active: true,
                references: files.iter().map(|file| Reference::file(file)).collect(),
            });
            self
        }

        fn inactive(mut self) -> Fixture {
            self.items.last_mut().unwrap().active = false;
            self
        }

        fn accepted(mut self, uid: &str, file: &str, fingerprint: &str) -> Fixture {
            self.locked
                .entry(uid.to_string())
                .or_default()
                .insert(format!("file:{file}"), fingerprint.to_string());
            self
        }

        fn accepted_as_is(self, uid: &str, file: &str) -> Fixture {
            let contents = fs::read(self.project.path().join(file)).unwrap();
            let fingerprint = fingerprint::sha256(&contents);
            self.accepted(uid, file, &fingerprint)
        }

        fn check(&self, options: CheckOptions) -> Vec<Problem> {
            let tree = Tree {
                root: self.project.path().to_path_buf(),
                items: self
                    .items
                    .iter()
                    .map(|item| (item.uid.clone(), item.clone()))
                    .collect::<BTreeMap<_, _>>(),
            };
            check(&tree, &self.locked, &options).unwrap()
        }
    }

    fn file(path: &str) -> Justification {
        Justification::File {
            path: path.to_string(),
        }
    }

    #[test]
    fn accepted_unchanged_justification_has_no_problems() {
        let fixture = Fixture::new()
            .requirement("REQ001", &["a.rs"])
            .accepted_as_is("REQ001", "a.rs");
        assert_eq!(fixture.check(CheckOptions::default()), vec![]);
    }

    #[test]
    fn changed_justification_is_reported() {
        let fixture = Fixture::new().requirement("REQ001", &["a.rs"]).accepted(
            "REQ001",
            "a.rs",
            "sha256:old",
        );
        assert_eq!(
            fixture.check(CheckOptions::default()),
            vec![Problem::Changed {
                requirement: "REQ001".into(),
                justification: file("a.rs")
            }]
        );
    }

    #[test]
    fn unaccepted_justification_is_reported() {
        let fixture = Fixture::new().requirement("REQ001", &["a.rs"]);
        assert_eq!(
            fixture.check(CheckOptions::default()),
            vec![Problem::Unaccepted {
                requirement: "REQ001".into(),
                justification: file("a.rs")
            }]
        );
    }

    #[test]
    fn missing_justification_is_reported() {
        let fixture = Fixture::new()
            .requirement("REQ001", &["a.rs"])
            .accepted_as_is("REQ001", "a.rs");
        fs::remove_file(fixture.project.path().join("a.rs")).unwrap();
        assert_eq!(
            fixture.check(CheckOptions::default()),
            vec![Problem::Missing {
                requirement: "REQ001".into(),
                justification: file("a.rs")
            }]
        );
    }

    #[test]
    fn inactive_requirements_are_not_checked() {
        let fixture = Fixture::new().requirement("REQ001", &["a.rs"]).inactive();
        assert_eq!(fixture.check(CheckOptions::default()), vec![]);
    }

    #[test]
    fn unjustified_requirement_passes_by_default() {
        let fixture = Fixture::new().requirement("REQ001", &[]);
        assert_eq!(fixture.check(CheckOptions::default()), vec![]);
    }

    #[test]
    fn unjustified_requirement_is_reported_when_justification_is_required() {
        let fixture = Fixture::new().requirement("REQ001", &[]);
        let options = CheckOptions {
            require_justification: true,
        };
        assert_eq!(
            fixture.check(options),
            vec![Problem::Unjustified {
                requirement: "REQ001".into()
            }]
        );
    }

    #[test]
    fn inactive_unjustified_requirement_is_not_reported() {
        let fixture = Fixture::new().requirement("REQ001", &[]).inactive();
        let options = CheckOptions {
            require_justification: true,
        };
        assert_eq!(fixture.check(options), vec![]);
    }

    #[test]
    fn accepted_fingerprints_of_a_removed_requirement_are_reported() {
        let fixture = Fixture::new().accepted("REQ001", "a.rs", "sha256:old");
        assert_eq!(
            fixture.check(CheckOptions::default()),
            vec![Problem::RequirementRemoved {
                requirement: "REQ001".into()
            }]
        );
    }

    #[test]
    fn accepted_fingerprints_of_an_inactive_requirement_are_not_reported_as_removed() {
        let fixture = Fixture::new()
            .requirement("REQ001", &["a.rs"])
            .inactive()
            .accepted("REQ001", "a.rs", "sha256:old");
        assert_eq!(fixture.check(CheckOptions::default()), vec![]);
    }

    #[test]
    fn every_problem_is_reported() {
        let fixture = Fixture::new()
            .requirement("REQ001", &["a.rs"])
            .requirement("REQ002", &["b.rs"]);
        assert_eq!(fixture.check(CheckOptions::default()).len(), 2);
    }

    #[test]
    fn problems_are_described_for_people() {
        let changed = Problem::Changed {
            requirement: "REQ001".into(),
            justification: file("a.rs"),
        };
        assert_eq!(
            changed.to_string(),
            "REQ001: file:a.rs changed since it was accepted"
        );
        let removed = Problem::RequirementRemoved {
            requirement: "REQ001".into(),
        };
        assert_eq!(removed.to_string(), "REQ001: requirement no longer exists");
    }
}
