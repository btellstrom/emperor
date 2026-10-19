//! Read-only loader for a Doorstop requirement tree.
//!
//! Doorstop owns these files; emperor never writes them.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result, read_to_string};

const DOCUMENT_FILE: &str = ".doorstop.yml";
const SKIP_FILE: &str = ".doorstop.skip";

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
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub keyword: Option<String>,
}

#[cfg(test)]
impl Reference {
    pub fn file(path: &str) -> Reference {
        Reference {
            path: path.to_string(),
            kind: "file".to_string(),
            keyword: None,
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
            for item in load_items(&document.dir)? {
                items.insert(item.uid.clone(), item);
            }
        }

        Ok(Tree {
            root: root.to_path_buf(),
            items,
        })
    }
}

fn find_documents(dir: &Path, documents: &mut Vec<Document>) -> Result<()> {
    if dir.join(SKIP_FILE).exists() {
        return Ok(());
    }

    let config = dir.join(DOCUMENT_FILE);
    if config.is_file() {
        ensure_yaml_items(&config)?;
        documents.push(Document {
            dir: dir.to_path_buf(),
        });
    }

    for path in subdirectories(dir)? {
        find_documents(&path, documents)?;
    }
    Ok(())
}

fn ensure_yaml_items(config: &Path) -> Result<()> {
    let file: DocumentFile =
        serde_yaml_ng::from_str(&read_to_string(config)?).map_err(Error::parse(config))?;
    let settings = file.settings;
    // Items in any other format would be skipped, so a check would pass without checking them.
    match settings.itemformat.as_deref() {
        None | Some("yaml") => Ok(()),
        Some("markdown") => Err(Error::MarkdownItems {
            prefix: settings.prefix,
        }),
        Some(format) => Err(Error::UnknownItemFormat {
            format: format.to_string(),
            prefix: settings.prefix,
        }),
    }
}

fn subdirectories(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut subdirectories = Vec::new();
    for entry in fs::read_dir(dir).map_err(Error::io(dir))? {
        let path = entry.map_err(Error::io(dir))?.path();
        let hidden = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if path.is_dir() && !hidden {
            subdirectories.push(path);
        }
    }
    Ok(subdirectories)
}

fn load_items(dir: &Path) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    for entry in fs::read_dir(dir).map_err(Error::io(dir))? {
        let path = entry.map_err(Error::io(dir))?.path();
        if path.extension().is_some_and(|ext| ext == "yml") && !path.ends_with(DOCUMENT_FILE) {
            items.push(load_item(&path)?);
        }
    }
    Ok(items)
}

fn load_item(path: &Path) -> Result<Item> {
    let file: ItemFile =
        serde_yaml_ng::from_str(&read_to_string(path)?).map_err(Error::parse(path))?;
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
