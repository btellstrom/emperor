//! A justification is a piece of evidence that a requirement is fulfilled.

use std::fmt;

use crate::doorstop::Item;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Justification {
    /// A whole file, compared byte for byte.
    File { path: String },
}

impl fmt::Display for Justification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Justification::File { path } => write!(f, "file:{path}"),
        }
    }
}

/// The justifications a Doorstop item names itself.
pub fn of_item(item: &Item) -> Vec<Justification> {
    item.references
        .iter()
        .map(|reference| Justification::File {
            path: reference.path.clone(),
        })
        .collect()
}
