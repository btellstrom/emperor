//! Read-only loader for a Doorstop requirement tree.
//!
//! Doorstop owns these files; emperor never writes them.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result, read_to_string};

const DOCUMENT_FILE: &str = ".doorstop.yml";
/// Skips the document in the folder that contains it.
const SKIP_DOCUMENT_FILE: &str = ".doorstop.skip";
/// Skips the folder that contains it and everything below.
const SKIP_ALL_FILE: &str = ".doorstop.skip-all";
/// Folders Doorstop never searches for documents.
const EXCLUDED_FOLDERS: [&str; 4] = [".git", ".tox", ".venv", "venv"];

#[derive(Debug, Deserialize)]
struct DocumentFile {
    settings: Settings,
}

#[derive(Debug, Deserialize)]
struct Settings {
    prefix: String,
    /// Doorstop defaults to YAML when this is missing.
    itemformat: Option<String>,
}

#[derive(Debug)]
struct Document {
    dir: PathBuf,
    prefix: String,
}

impl Document {
    /// Doorstop names item files after their UID, which starts with the document's prefix.
    fn is_item_file(&self, path: &Path) -> bool {
        let is_yaml = path.extension().is_some_and(|extension| extension == "yml");
        let named_like_an_item = path
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().starts_with(&self.prefix));
        is_yaml && named_like_an_item
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[cfg(test)]
impl Reference {
    pub fn file(path: &str) -> Reference {
        Reference {
            path: path.to_string(),
            kind: "file".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ItemFile {
    #[serde(default = "default_active")]
    active: bool,
    references: Option<Vec<Reference>>,
}

fn default_active() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct Item {
    pub uid: String,
    pub active: bool,
    pub references: Vec<Reference>,
}

#[derive(Debug)]
pub struct Tree {
    pub root: PathBuf,
    /// Items keyed by UID, so lookup by ID is direct.
    pub items: BTreeMap<String, Item>,
}

impl Tree {
    pub fn load(root: &Path) -> Result<Tree> {
        let mut documents = Vec::new();
        find_documents(root, &mut documents)?;
        if documents.is_empty() {
            return Err(Error::NoDocuments(root.to_path_buf()));
        }

        let mut items = BTreeMap::new();
        for document in &documents {
            load_items(document, &document.dir, &mut items)?;
        }

        Ok(Tree {
            root: root.to_path_buf(),
            items,
        })
    }
}

fn find_documents(dir: &Path, documents: &mut Vec<Document>) -> Result<()> {
    if dir.join(SKIP_ALL_FILE).exists() {
        return Ok(());
    }
    let config = dir.join(DOCUMENT_FILE);
    if config.is_file() && !dir.join(SKIP_DOCUMENT_FILE).exists() {
        documents.push(load_document(dir, &config)?);
    }
    for subdirectory in subdirectories(dir)? {
        find_documents(&subdirectory, documents)?;
    }
    Ok(())
}

fn load_document(dir: &Path, config: &Path) -> Result<Document> {
    let file: DocumentFile = read_yaml(config)?;
    let prefix = file.settings.prefix;
    // Items in any other format would be skipped, so a check would pass without checking them.
    match file.settings.itemformat.as_deref() {
        None | Some("yaml") => Ok(Document {
            dir: dir.to_path_buf(),
            prefix,
        }),
        Some("markdown") => Err(Error::MarkdownItems { prefix }),
        Some(format) => Err(Error::UnknownItemFormat {
            format: format.to_string(),
            prefix,
        }),
    }
}

/// The subdirectories Doorstop searches.
fn subdirectories(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut subdirectories = Vec::new();
    for path in entries(dir)? {
        let excluded = path
            .file_name()
            .is_some_and(|name| EXCLUDED_FOLDERS.iter().any(|excluded| name == *excluded));
        if path.is_dir() && !excluded {
            subdirectories.push(path);
        }
    }
    Ok(subdirectories)
}

fn entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(Error::io(dir))? {
        entries.push(entry.map_err(Error::io(dir))?.path());
    }
    Ok(entries)
}

/// The items in a document's folder and its subfolders, except those of embedded documents.
fn load_items(document: &Document, dir: &Path, items: &mut BTreeMap<String, Item>) -> Result<()> {
    for path in entries(dir)? {
        if path.is_dir() {
            if !path.join(DOCUMENT_FILE).exists() {
                load_items(document, &path, items)?;
            }
        } else if document.is_item_file(&path) {
            let item = load_item(&path)?;
            if items.contains_key(&item.uid) {
                return Err(Error::DuplicateRequirement(item.uid));
            }
            items.insert(item.uid.clone(), item);
        }
    }
    Ok(())
}

fn load_item(path: &Path) -> Result<Item> {
    let file: ItemFile = read_yaml(path)?;
    let uid = path
        .file_stem()
        .expect("item file has a name")
        .to_string_lossy()
        .into_owned();
    Ok(Item {
        uid,
        active: file.active,
        references: file.references.unwrap_or_default(),
    })
}

fn read_yaml<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_yaml_ng::from_str(&read_to_string(path)?).map_err(Error::parse(path))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// A project folder in which tests lay out Doorstop documents and items.
    struct Layout(TempDir);

    impl Layout {
        fn new() -> Layout {
            Layout(TempDir::new().unwrap())
        }

        fn file(self, path: &str, contents: &str) -> Layout {
            let path = self.0.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
            self
        }

        fn document(self, dir: &str, prefix: &str) -> Layout {
            let settings = format!("settings:\n  digits: 3\n  prefix: {prefix}\n  sep: ''\n");
            self.file(&format!("{dir}/{DOCUMENT_FILE}"), &settings)
        }

        fn item(self, path: &str) -> Layout {
            self.file(path, "active: true\ntext: |\n  A requirement\n")
        }

        fn uids(&self) -> Vec<String> {
            Tree::load(self.0.path())
                .unwrap()
                .items
                .into_keys()
                .collect()
        }
    }

    #[test]
    fn finds_documents_in_hidden_folders() {
        let layout = Layout::new()
            .document(".github/reqs", "REQ")
            .item(".github/reqs/REQ001.yml");
        assert_eq!(layout.uids(), ["REQ001"]);
    }

    #[test]
    fn ignores_the_folders_doorstop_ignores() {
        let mut layout = Layout::new()
            .document("reqs", "REQ")
            .item("reqs/REQ001.yml");
        for folder in [".git", ".tox", ".venv", "venv"] {
            layout = layout
                .document(&format!("{folder}/reqs"), "TMP")
                .item(&format!("{folder}/reqs/TMP001.yml"));
        }
        assert_eq!(layout.uids(), ["REQ001"]);
    }

    #[test]
    fn skip_all_marker_skips_a_folder_and_everything_below_it() {
        let layout = Layout::new()
            .document("reqs", "REQ")
            .item("reqs/REQ001.yml")
            .file("vendor/.doorstop.skip-all", "")
            .document("vendor/deep/tmp", "TMP")
            .item("vendor/deep/tmp/TMP001.yml");
        assert_eq!(layout.uids(), ["REQ001"]);
    }

    #[test]
    fn skip_marker_in_a_document_folder_skips_that_document() {
        let layout = Layout::new()
            .document("reqs", "REQ")
            .item("reqs/REQ001.yml")
            .document("tmp", "TMP")
            .item("tmp/TMP001.yml")
            .file("tmp/.doorstop.skip", "");
        assert_eq!(layout.uids(), ["REQ001"]);
    }

    #[test]
    fn skip_marker_outside_a_document_folder_has_no_effect() {
        let layout = Layout::new()
            .file("vendor/.doorstop.skip", "")
            .document("vendor/tmp", "TMP")
            .item("vendor/tmp/TMP001.yml");
        assert_eq!(layout.uids(), ["TMP001"]);
    }

    #[test]
    fn loads_items_from_subfolders_of_a_document() {
        let layout = Layout::new()
            .document("reqs", "REQ")
            .item("reqs/login/REQ001.yml");
        assert_eq!(layout.uids(), ["REQ001"]);
    }

    #[test]
    fn loads_the_items_of_a_document_inside_another_document() {
        let layout = Layout::new()
            .document("reqs", "REQ")
            .item("reqs/REQ001.yml")
            .document("reqs/low", "LLR")
            .item("reqs/low/LLR001.yml");
        assert_eq!(layout.uids(), ["LLR001", "REQ001"]);
    }

    #[test]
    fn the_same_uid_in_two_places_is_an_error() {
        let layout = Layout::new()
            .document("reqs", "REQ")
            .item("reqs/REQ001.yml")
            .item("reqs/old/REQ001.yml");
        let error = Tree::load(layout.0.path()).unwrap_err();
        assert!(matches!(error, Error::DuplicateRequirement(uid) if uid == "REQ001"));
    }

    #[test]
    fn ignores_files_that_are_not_items_of_the_document() {
        let layout = Layout::new()
            .document("reqs", "REQ")
            .item("reqs/REQ001.yml")
            .file("reqs/notes.yml", "- not an item\n")
            .file("reqs/README.md", "# Requirements\n");
        assert_eq!(layout.uids(), ["REQ001"]);
    }
}
