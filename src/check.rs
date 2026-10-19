//! Compares the current justifications with the fingerprints accepted in the lockfile.

use crate::doorstop::Tree;
use crate::error::{Error, Result};
use crate::evidence::Evidence;
use crate::fingerprint;
use crate::justification::Justification;
use crate::lockfile::Fingerprints;
pub use crate::problem::Problem;

#[derive(Debug, Default)]
pub struct CheckOptions {
    /// Report active requirements that have no justifications at all.
    pub require_justification: bool,
}

/// Every problem in the tree, in requirement order, followed by problems with tags.
pub fn check(
    tree: &Tree,
    evidence: &Evidence,
    locked: &Fingerprints,
    options: &CheckOptions,
) -> Result<Vec<Problem>> {
    let mut problems = Vec::new();
    for item in tree.items.values().filter(|item| item.active) {
        let justifications: Vec<&Justification> = evidence.of(&item.uid).collect();
        if justifications.is_empty() && options.require_justification {
            problems.push(Problem::Unjustified {
                requirement: item.uid.clone(),
            });
        }
        for justification in &justifications {
            problems.extend(check_justification(
                tree,
                locked,
                &item.uid,
                (*justification).clone(),
            )?);
        }
        problems.extend(dropped_justifications(locked, &item.uid, &justifications)?);
    }
    problems.extend(removed_requirements(tree, locked));
    problems.extend(evidence.problems.iter().cloned());
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

/// Accepted justifications the requirement no longer has.
fn dropped_justifications(
    locked: &Fingerprints,
    uid: &str,
    current: &[&Justification],
) -> Result<Vec<Problem>> {
    let accepted_keys = locked
        .get(uid)
        .into_iter()
        .flat_map(|fingerprints| fingerprints.keys());
    let mut dropped = Vec::new();
    for key in accepted_keys {
        if current
            .iter()
            .any(|justification| justification.key() == *key)
        {
            continue;
        }
        let justification =
            Justification::from_key(key).ok_or_else(|| Error::UnknownLockfileKey(key.clone()))?;
        dropped.push(Problem::Dropped {
            requirement: uid.to_string(),
            justification,
        });
    }
    Ok(dropped)
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
    use crate::evidence;
    use crate::test_support::{Project, file, locked};

    fn check_with(project: &Project, locked: &Fingerprints, options: CheckOptions) -> Vec<Problem> {
        let tree = project.tree();
        let evidence = evidence::gather(&tree).unwrap();
        check(&tree, &evidence, locked, &options).unwrap()
    }

    fn check_project(project: &Project, locked: &Fingerprints) -> Vec<Problem> {
        check_with(project, locked, CheckOptions::default())
    }

    fn requiring_justification(project: &Project) -> Vec<Problem> {
        let options = CheckOptions {
            require_justification: true,
        };
        check_with(project, &Fingerprints::new(), options)
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
    fn a_tagged_function_is_checked() {
        let project = Project::new()
            .requirement("REQ001", &[])
            .file("lib.rs", "#[justifies(\"REQ001\")]\nfn login() {}\n");
        assert_eq!(
            check_project(&project, &Fingerprints::new()),
            vec![Problem::Unaccepted {
                requirement: "REQ001".into(),
                justification: Justification::RustFn {
                    path: "lib.rs".into(),
                    item: "login".into()
                }
            }]
        );
    }

    #[test]
    fn problems_with_tags_are_reported() {
        let project = Project::new()
            .requirement("REQ001", &[])
            .file("lib.rs", "#[justifies(\"REQ999\")]\nfn login() {}\n");
        let problems = check_project(&project, &Fingerprints::new());
        assert!(matches!(
            &problems[..],
            [Problem::UnknownRequirement { .. }]
        ));
    }

    #[test]
    fn an_accepted_justification_the_requirement_no_longer_has_is_reported() {
        let project = Project::new().requirement("REQ001", &[]);
        let locked = locked(&[(
            "REQ001",
            "gherkin:features/a.feature::Old name",
            "sha256:old",
        )]);
        assert_eq!(
            check_project(&project, &locked),
            vec![Problem::Dropped {
                requirement: "REQ001".into(),
                justification: Justification::Gherkin {
                    path: "features/a.feature".into(),
                    scenario: "Old name".into()
                }
            }]
        );
    }

    #[test]
    fn every_problem_is_reported() {
        let project = Project::new()
            .requirement("REQ001", &["a.rs"])
            .requirement("REQ002", &["b.rs"]);
        assert_eq!(check_project(&project, &Fingerprints::new()).len(), 2);
    }
}
