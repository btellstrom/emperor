//! Something `check` found wrong with a requirement's justifications.

use std::fmt;

use serde::Serialize;

use crate::justification::Justification;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    /// An accepted justification the requirement no longer has, e.g. a renamed scenario.
    Dropped {
        requirement: String,
        justification: Justification,
    },
    /// A test justifying the requirement failed.
    TestFailed {
        requirement: String,
        justification: Justification,
    },
    /// A test justifying the requirement was skipped.
    TestSkipped {
        requirement: String,
        justification: Justification,
    },
    /// A test justifying the requirement is not in the test results.
    TestNotRun {
        requirement: String,
        justification: Justification,
    },
    /// The requirement has no justifications, and justifications are required.
    Unjustified { requirement: String },
    /// The lockfile has fingerprints for a requirement that no longer exists.
    RequirementRemoved { requirement: String },
    /// A tag in code names a requirement that does not exist.
    UnknownRequirement {
        requirement: String,
        location: Location,
    },
    /// A tag in code that emperor cannot use.
    InvalidTag { location: Location, reason: String },
}

/// A line in a file of the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Location {
    /// Relative to the project root, with `/` separators.
    pub path: String,
    pub line: usize,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.path, self.line)
    }
}

impl Problem {
    /// The existing requirement the problem is about.
    pub fn requirement(&self) -> Option<&str> {
        match self {
            Problem::Changed { requirement, .. }
            | Problem::Unaccepted { requirement, .. }
            | Problem::Missing { requirement, .. }
            | Problem::Dropped { requirement, .. }
            | Problem::TestFailed { requirement, .. }
            | Problem::TestSkipped { requirement, .. }
            | Problem::TestNotRun { requirement, .. }
            | Problem::Unjustified { requirement }
            | Problem::RequirementRemoved { requirement } => Some(requirement),
            Problem::UnknownRequirement { .. } | Problem::InvalidTag { .. } => None,
        }
    }

    /// Where in the code the problem is, for problems found in tags.
    pub fn location(&self) -> Option<&Location> {
        match self {
            Problem::UnknownRequirement { location, .. } | Problem::InvalidTag { location, .. } => {
                Some(location)
            }
            _ => None,
        }
    }

    pub fn justification(&self) -> Option<&Justification> {
        match self {
            Problem::Changed { justification, .. }
            | Problem::Unaccepted { justification, .. }
            | Problem::Missing { justification, .. }
            | Problem::Dropped { justification, .. }
            | Problem::TestFailed { justification, .. }
            | Problem::TestSkipped { justification, .. }
            | Problem::TestNotRun { justification, .. } => Some(justification),
            _ => None,
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
            Problem::Missing { justification, .. } | Problem::Dropped { justification, .. } => {
                format!("{justification} not found")
            }
            Problem::TestFailed { justification, .. } => {
                format!("test {} failed", test_name(justification))
            }
            Problem::TestSkipped { justification, .. } => {
                format!("test {} was skipped", test_name(justification))
            }
            Problem::TestNotRun { justification, .. } => {
                format!("test {} did not run", test_name(justification))
            }
            Problem::Unjustified { .. } => "has no justifications".to_string(),
            Problem::RequirementRemoved { .. } => "requirement no longer exists".to_string(),
            Problem::UnknownRequirement { requirement, .. } => {
                format!("unknown requirement {requirement}")
            }
            Problem::InvalidTag { reason, .. } => reason.clone(),
        }
    }

    /// Whether running `emperor accept` after a review resolves the problem.
    pub fn is_resolved_by_accepting(&self) -> bool {
        matches!(
            self,
            Problem::Changed { .. }
                | Problem::Unaccepted { .. }
                | Problem::Dropped { .. }
                | Problem::RequirementRemoved { .. }
        )
    }
}

/// How a test justification is named in messages: its module path within its file.
fn test_name(justification: &Justification) -> String {
    match justification {
        Justification::RustFn { item, .. } => item.clone(),
        other => other.to_string(),
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.requirement(), self.location()) {
            (Some(requirement), _) => write!(f, "{requirement}: {}", self.description()),
            (None, Some(location)) => write!(f, "{location}: {}", self.description()),
            (None, None) => write!(f, "{}", self.description()),
        }
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

    fn location(path: &str, line: usize) -> Location {
        Location {
            path: path.into(),
            line,
        }
    }

    #[test]
    fn tag_problems_are_described_by_their_location() {
        let unknown = Problem::UnknownRequirement {
            requirement: "REQ999".into(),
            location: location("tests/login.rs", 2),
        };
        assert_eq!(
            unknown.to_string(),
            "tests/login.rs:2: unknown requirement REQ999"
        );
        let invalid = Problem::InvalidTag {
            location: location("src/auth.rs", 1),
            reason: "justifies is only supported on functions".into(),
        };
        assert_eq!(
            invalid.to_string(),
            "src/auth.rs:1: justifies is only supported on functions"
        );
    }

    #[test]
    fn tag_problems_are_about_no_existing_requirement() {
        let unknown = Problem::UnknownRequirement {
            requirement: "REQ999".into(),
            location: location("tests/login.rs", 2),
        };
        assert_eq!(unknown.requirement(), None);
        assert_eq!(unknown.location(), Some(&location("tests/login.rs", 2)));
        assert!(!unknown.is_resolved_by_accepting());
    }

    #[test]
    fn a_dropped_justification_is_no_longer_found_and_accepting_forgets_it() {
        let dropped = Problem::Dropped {
            requirement: "REQ001".into(),
            justification: file("a.rs"),
        };
        assert_eq!(dropped.to_string(), "REQ001: file:a.rs not found");
        assert!(dropped.is_resolved_by_accepting());
    }

    #[test]
    fn test_problems_name_the_test() {
        let failed = Problem::TestFailed {
            requirement: "REQ001".into(),
            justification: Justification::RustFn {
                path: "tests/login.rs".into(),
                item: "rejects".into(),
            },
        };
        assert_eq!(failed.to_string(), "REQ001: test rejects failed");
        assert!(!failed.is_resolved_by_accepting());
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
