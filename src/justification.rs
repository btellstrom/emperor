//! A justification is a piece of evidence that a requirement is fulfilled.

use std::fmt;

use serde::Serialize;

use crate::doorstop::{Item, Reference};
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Justification {
    /// A whole file, compared byte for byte.
    File { path: String },
    /// A Rust function, compared by its tokens.
    RustFn { path: String, item: String },
    /// A Gherkin scenario, compared by its steps.
    Gherkin { path: String, scenario: String },
}

impl fmt::Display for Justification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Justification::File { path } => write!(f, "file:{path}"),
            Justification::RustFn { path, item } => write!(f, "rust-fn:{path}::{item}"),
            Justification::Gherkin { path, scenario } => write!(f, "gherkin:{path}::{scenario}"),
        }
    }
}

impl Justification {
    /// The file the justification is in, relative to the project root.
    pub fn path(&self) -> &str {
        match self {
            Justification::File { path }
            | Justification::RustFn { path, .. }
            | Justification::Gherkin { path, .. } => path,
        }
    }

    /// Identifies the justification in the lockfile.
    pub fn key(&self) -> String {
        self.to_string()
    }

    /// The justification a lockfile key identifies, or `None` for an unknown kind.
    ///
    /// Paths never contain `::`, so the first `::` ends the path.
    pub fn from_key(key: &str) -> Option<Justification> {
        let (kind, rest) = key.split_once(':')?;
        let path = |path: &str| path.to_string();
        match kind {
            "file" => Some(Justification::File { path: path(rest) }),
            "rust-fn" => {
                let (file, item) = rest.split_once("::")?;
                Some(Justification::RustFn {
                    path: path(file),
                    item: item.to_string(),
                })
            }
            "gherkin" => {
                let (file, scenario) = rest.split_once("::")?;
                Some(Justification::Gherkin {
                    path: path(file),
                    scenario: scenario.to_string(),
                })
            }
            _ => None,
        }
    }
}

/// The justifications a Doorstop item names itself.
pub fn of_item(item: &Item) -> Result<Vec<Justification>> {
    item.references
        .iter()
        .chain(&item.justified_by)
        .map(|reference| from_reference(&item.uid, reference))
        .collect()
}

fn from_reference(uid: &str, reference: &Reference) -> Result<Justification> {
    let path = reference.path.clone();
    match reference.kind.as_str() {
        "file" => Ok(Justification::File { path }),
        "rust-fn" => match &reference.item {
            Some(item) => Ok(Justification::RustFn {
                path,
                item: item.clone(),
            }),
            None => Err(Error::IncompleteJustification {
                requirement: uid.to_string(),
                kind: reference.kind.clone(),
            }),
        },
        kind => Err(Error::UnknownJustificationType {
            requirement: uid.to_string(),
            kind: kind.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(references: Vec<Reference>, justified_by: Vec<Reference>) -> Item {
        Item {
            uid: "REQ001".to_string(),
            active: true,
            references,
            justified_by,
        }
    }

    #[test]
    fn a_reference_is_a_file_justification() {
        let item = item(vec![Reference::new("file", "a.md", None)], vec![]);
        assert_eq!(
            of_item(&item).unwrap(),
            [Justification::File {
                path: "a.md".into()
            }]
        );
    }

    #[test]
    fn justified_by_names_rust_functions_and_files() {
        let item = item(
            vec![],
            vec![
                Reference::new("rust-fn", "tests/login.rs", Some("rejects")),
                Reference::new("file", "b.md", None),
            ],
        );
        assert_eq!(
            of_item(&item).unwrap(),
            [
                Justification::RustFn {
                    path: "tests/login.rs".into(),
                    item: "rejects".into()
                },
                Justification::File {
                    path: "b.md".into()
                },
            ]
        );
    }

    #[test]
    fn an_unknown_type_is_an_error() {
        let item = item(
            vec![],
            vec![Reference::new("python-fn", "t.py", Some("test"))],
        );
        let error = of_item(&item).unwrap_err();
        assert_eq!(
            error.to_string(),
            "REQ001: unknown justification type python-fn"
        );
    }

    #[test]
    fn a_rust_function_without_an_item_is_an_error() {
        let item = item(
            vec![],
            vec![Reference::new("rust-fn", "tests/login.rs", None)],
        );
        assert!(matches!(
            of_item(&item).unwrap_err(),
            Error::IncompleteJustification { .. }
        ));
    }

    #[test]
    fn a_key_identifies_its_justification() {
        let justifications = [
            Justification::File {
                path: "docs/a.md".into(),
            },
            Justification::RustFn {
                path: "src/auth.rs".into(),
                item: "tests::hashes".into(),
            },
            Justification::Gherkin {
                path: "features/login.feature".into(),
                scenario: "A: tricky :: name".into(),
            },
        ];
        for justification in justifications {
            assert_eq!(
                Justification::from_key(&justification.key()),
                Some(justification)
            );
        }
        assert_eq!(Justification::from_key("unknown:thing"), None);
    }

    #[test]
    fn justifications_are_described_by_type_and_location() {
        let function = Justification::RustFn {
            path: "tests/login.rs".into(),
            item: "tests::rejects".into(),
        };
        assert_eq!(
            function.to_string(),
            "rust-fn:tests/login.rs::tests::rejects"
        );
    }
}
