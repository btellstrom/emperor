//! Fixtures shared by the unit tests.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use crate::doorstop::{Item, Reference, Tree};
use crate::fingerprint;
use crate::justification::Justification;
use crate::lockfile::Fingerprints;

/// A temporary project with requirements whose justifications are files in it.
pub struct Project {
    dir: TempDir,
    items: BTreeMap<String, Item>,
}

impl Project {
    pub fn new() -> Project {
        Project {
            dir: TempDir::new().unwrap(),
            items: BTreeMap::new(),
        }
    }

    /// Adds an active requirement justified by the given files, creating them.
    pub fn requirement(mut self, uid: &str, files: &[&str]) -> Project {
        for file in files {
            fs::write(self.path(file), format!("contents of {file}")).unwrap();
        }
        let item = Item {
            uid: uid.to_string(),
            active: true,
            references: files
                .iter()
                .map(|file| Reference::new("file", file, None))
                .collect(),
            justified_by: Vec::new(),
        };
        self.items.insert(uid.to_string(), item);
        self
    }

    /// Writes a file into the project, creating its folder.
    pub fn file(self, path: &str, contents: &str) -> Project {
        let path = self.path(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        self
    }

    /// Adds a `justified_by` entry naming a Rust function to an existing requirement.
    pub fn justify_by_rust_fn(&mut self, uid: &str, path: &str, function: &str) {
        let reference = Reference::new("rust-fn", path, Some(function));
        self.items
            .get_mut(uid)
            .unwrap()
            .justified_by
            .push(reference);
    }

    pub fn inactive(mut self, uid: &str) -> Project {
        self.items.get_mut(uid).unwrap().active = false;
        self
    }

    pub fn path(&self, file: &str) -> PathBuf {
        self.dir.path().join(file)
    }

    pub fn tree(&self) -> Tree {
        Tree {
            root: self.dir.path().to_path_buf(),
            items: self.items.clone(),
        }
    }

    pub fn fingerprint_of(&self, file: &str) -> String {
        fingerprint::sha256(&fs::read(self.path(file)).unwrap())
    }
}

/// Lockfile contents from `(requirement, justification key, fingerprint)` entries.
pub fn locked(entries: &[(&str, &str, &str)]) -> Fingerprints {
    let mut locked = Fingerprints::new();
    for (uid, key, fingerprint) in entries {
        locked
            .entry(uid.to_string())
            .or_default()
            .insert(key.to_string(), fingerprint.to_string());
    }
    locked
}

pub fn file(path: &str) -> Justification {
    Justification::File {
        path: path.to_string(),
    }
}
