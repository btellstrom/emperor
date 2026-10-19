//! Verdicts already given, so an unchanged request is not judged again.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{JudgeConfig, Verdict};
use crate::error::{Error, Result};
use crate::files::read_source;
use crate::fingerprint::sha256;

const CACHE_FILE: &str = ".emperor/verdicts.json";

/// Identifies a request to a particular judge.
pub fn cache_key(config: &JudgeConfig, input: &str) -> String {
    let judged = serde_json::json!([config.command, input]).to_string();
    sha256(judged.as_bytes())
}

/// Verdicts by cache key, kept in `.emperor/verdicts.json`.
pub struct VerdictCache {
    path: PathBuf,
    pub verdicts: BTreeMap<String, Verdict>,
}

impl VerdictCache {
    pub fn load(root: &Path) -> Result<VerdictCache> {
        let path = root.join(CACHE_FILE);
        let verdicts = match read_source(&path)? {
            Some(json) => serde_json::from_str(&json).map_err(Error::parse(&path))?,
            None => BTreeMap::new(),
        };
        Ok(VerdictCache { path, verdicts })
    }

    pub fn save(&self) -> Result<()> {
        let folder = self.path.parent().expect("the cache file is in a folder");
        fs::create_dir_all(folder).map_err(Error::io(folder))?;
        let json = serde_json::to_string_pretty(&self.verdicts)
            .expect("verdicts always serialise to JSON");
        fs::write(&self.path, json + "\n").map_err(Error::io(&self.path))
    }
}
