//! Something `check` found wrong with a requirement's justifications.

use std::fmt;

use serde::Serialize;

use crate::justification::Justification;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
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

impl Problem {
    pub fn requirement(&self) -> &str {
        match self {
            Problem::Changed { requirement, .. }
            | Problem::Unaccepted { requirement, .. }
            | Problem::Missing { requirement, .. }
            | Problem::Unjustified { requirement }
            | Problem::RequirementRemoved { requirement } => requirement,
        }
    }

    pub fn justification(&self) -> Option<&Justification> {
        match self {
            Problem::Changed { justification, .. }
            | Problem::Unaccepted { justification, .. }
            | Problem::Missing { justification, .. } => Some(justification),
            Problem::Unjustified { .. } | Problem::RequirementRemoved { .. } => None,
        }
    }

    /// What is wrong, without naming the requirement.
    pub fn description(&self) -> String {
        match self {
            Problem::Changed { justification, .. } => {
                format!("{justification} changed since it was accepted")
            }
            Problem::Unaccepted { justification, .. } => {
                format!("{justification} has not been accepted")
            }
            Problem::Missing { justification, .. } => format!("{justification} not found"),
            Problem::Unjustified { .. } => "has no justifications".to_string(),
            Problem::RequirementRemoved { .. } => "requirement no longer exists".to_string(),
        }
    }

    /// Whether running `emperor accept` after a review resolves the problem.
    pub fn is_resolved_by_accepting(&self) -> bool {
        matches!(
            self,
            Problem::Changed { .. }
                | Problem::Unaccepted { .. }
                | Problem::RequirementRemoved { .. }
        )
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.requirement(), self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::file;

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

    #[test]
    fn only_some_problems_are_resolved_by_accepting() {
        let missing = Problem::Missing {
            requirement: "REQ001".into(),
            justification: file("a.rs"),
        };
        let unjustified = Problem::Unjustified {
            requirement: "REQ001".into(),
        };
        assert!(!missing.is_resolved_by_accepting());
        assert!(!unjustified.is_resolved_by_accepting());
    }
}
