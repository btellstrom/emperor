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
        let justifications = justification::of_item(item)?;
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
        .and_then(|fingerprints| fingerprints.get(&justification.key()));
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
    use std::fs;

    use super::*;
    use crate::test_support::{Project, file, locked};

    fn check_project(project: &Project, locked: &Fingerprints) -> Vec<Problem> {
        check(&project.tree(), locked, &CheckOptions::default()).unwrap()
    }

    fn requiring_justification(project: &Project) -> Vec<Problem> {
        let options = CheckOptions {
            require_justification: true,
        };
        check(&project.tree(), &Fingerprints::new(), &options).unwrap()
    }

    #[test]
    fn accepted_unchanged_justification_has_no_problems() {
        let project = Project::new().requirement("REQ001", &["a.rs"]);
        let locked = locked(&[("REQ001", "file:a.rs", &project.fingerprint_of("a.rs"))]);
        assert_eq!(check_project(&project, &locked), vec![]);
    }

    #[test]
    fn changed_justification_is_reported() {
        let project = Project::new().requirement("REQ001", &["a.rs"]);
        let locked = locked(&[("REQ001", "file:a.rs", "sha256:old")]);
        assert_eq!(
            check_project(&project, &locked),
            vec![Problem::Changed {
                requirement: "REQ001".into(),
                justification: file("a.rs")
            }]
        );
    }

    #[test]
    fn unaccepted_justification_is_reported() {
        let project = Project::new().requirement("REQ001", &["a.rs"]);
        assert_eq!(
            check_project(&project, &Fingerprints::new()),
            vec![Problem::Unaccepted {
                requirement: "REQ001".into(),
                justification: file("a.rs")
            }]
        );
    }

    #[test]
    fn missing_justification_is_reported() {
        let project = Project::new().requirement("REQ001", &["a.rs"]);
        let locked = locked(&[("REQ001", "file:a.rs", &project.fingerprint_of("a.rs"))]);
        fs::remove_file(project.path("a.rs")).unwrap();
        assert_eq!(
            check_project(&project, &locked),
            vec![Problem::Missing {
                requirement: "REQ001".into(),
                justification: file("a.rs")
            }]
        );
    }

    #[test]
    fn inactive_requirements_are_not_checked() {
        let project = Project::new()
            .requirement("REQ001", &["a.rs"])
            .inactive("REQ001");
        assert_eq!(check_project(&project, &Fingerprints::new()), vec![]);
    }

    #[test]
    fn unjustified_requirement_passes_by_default() {
        let project = Project::new().requirement("REQ001", &[]);
        assert_eq!(check_project(&project, &Fingerprints::new()), vec![]);
    }

    #[test]
    fn unjustified_requirement_is_reported_when_justification_is_required() {
        let project = Project::new().requirement("REQ001", &[]);
        assert_eq!(
            requiring_justification(&project),
            vec![Problem::Unjustified {
                requirement: "REQ001".into()
            }]
        );
    }

    #[test]
    fn inactive_unjustified_requirement_is_not_reported() {
        let project = Project::new().requirement("REQ001", &[]).inactive("REQ001");
        assert_eq!(requiring_justification(&project), vec![]);
    }

    #[test]
    fn accepted_fingerprints_of_a_removed_requirement_are_reported() {
        let locked = locked(&[("REQ001", "file:a.rs", "sha256:old")]);
        assert_eq!(
            check_project(&Project::new(), &locked),
            vec![Problem::RequirementRemoved {
                requirement: "REQ001".into()
            }]
        );
    }

    #[test]
    fn accepted_fingerprints_of_an_inactive_requirement_are_not_reported_as_removed() {
        let project = Project::new()
            .requirement("REQ001", &["a.rs"])
            .inactive("REQ001");
        let locked = locked(&[("REQ001", "file:a.rs", "sha256:old")]);
        assert_eq!(check_project(&project, &locked), vec![]);
    }

    #[test]
    fn every_problem_is_reported() {
        let project = Project::new()
            .requirement("REQ001", &["a.rs"])
            .requirement("REQ002", &["b.rs"]);
        assert_eq!(check_project(&project, &Fingerprints::new()).len(), 2);
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
