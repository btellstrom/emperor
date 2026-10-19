//! Reading project files, with the path attached to every error.

use std::fs;
use std::io;
use std::path::Path;

use crate::error::{Error, Result};

pub fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(Error::io(path))
}

/// The contents of a file, or `None` when it does not exist.
pub fn read_if_present(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io(path)(error)),
    }
}

/// The text of a source file, or `None` when it does not exist.
pub fn read_source(path: &Path) -> Result<Option<String>> {
    let Some(contents) = read_if_present(path)? else {
        return Ok(None);
    };
    String::from_utf8(contents)
        .map(Some)
        .map_err(Error::parse(path))
}
