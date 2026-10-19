//! Read-only loader for a Doorstop requirement tree.
//!
//! Doorstop owns these files; emperor never writes them.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Result;

const DOCUMENT_FILE: &str = ".doorstop.yml";
const SKIP_FILE: &str = ".doorstop.skip";

#[derive(Debug, Deserialize)]
struct DocumentFile {
    settings: Settings,
}

#[derive(Debug, Deserialize)]
struct Settings {
    prefix: String,
    parent: Option<String>,
    /// Doorstop defaults to YAML when this is missing.
    itemformat: Option<String>,
}

#[derive(Debug)]
pub struct Document {
    pub prefix: String,
    pub parent: Option<String>,
    pub dir: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Reference {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ItemFile {
    #[serde(default = "default_active")]
    active: bool,
    #[serde(default)]
    references: Option<Vec<Reference>>,
}

fn default_active() -> bool {
    true
}

#[derive(Debug)]
pub struct Item {
    pub uid: String,
    pub active: bool,
    pub references: Vec<Reference>,
}

#[derive(Debug)]
pub struct Tree {
    pub root: PathBuf,
    pub documents: Vec<Document>,
    /// Items keyed by UID, so lookup by ID is direct.
    pub items: BTreeMap<String, Item>,
}

impl Tree {
    pub fn load(root: &Path) -> Result<Tree> {
        let mut documents = Vec::new();
        find_documents(root, &mut documents)?;
        if documents.is_empty() {
            return Err(format!("no Doorstop documents found under {}", root.display()).into());
        }

        let mut items = BTreeMap::new();
        for document in &documents {
            for item in load_items(&document.dir)? {
                items.insert(item.uid.clone(), item);
            }
        }

        Ok(Tree {
            root: root.to_path_buf(),
            documents,
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
        let file: DocumentFile = serde_yaml_ng::from_str(&fs::read_to_string(&config)?)
            .map_err(|e| format!("{}: {e}", config.display()))?;
        // Items in any other format would be skipped silently, so a check would pass
        // without checking them.
        match file.settings.itemformat.as_deref() {
            None | Some("yaml") => {}
            Some("markdown") => {
                return Err(format!("{}: Markdown items are not supported", file.settings.prefix).into());
            }
            Some(other) => {
                return Err(format!("{}: unknown item format {other}", file.settings.prefix).into());
            }
        }
        documents.push(Document {
            prefix: file.settings.prefix,
            parent: file.settings.parent,
            dir: dir.to_path_buf(),
        });
    }

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let hidden = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if path.is_dir() && !hidden {
            find_documents(&path, documents)?;
        }
    }
    Ok(())
}

fn load_items(dir: &Path) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|ext| ext != "yml") || path.ends_with(DOCUMENT_FILE) {
            continue;
        }
        let file: ItemFile = serde_yaml_ng::from_str(&fs::read_to_string(&path)?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let uid = path
            .file_stem()
            .expect("item file has a name")
            .to_string_lossy()
            .into_owned();
        items.push(Item {
            uid,
            active: file.active,
            references: file.references.unwrap_or_default(),
        });
    }
    Ok(items)
}
