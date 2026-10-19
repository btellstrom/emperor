//! Where justifications are, for annotating them in CI.

use std::fs;
use std::path::Path;

use crate::justification::Justification;
use crate::rust_source;

/// The line a justification starts on. Best effort: `None` when it has no line or
/// cannot be read, since a missing line only makes an annotation less precise.
pub fn line_of(root: &Path, justification: &Justification) -> Option<usize> {
    match justification {
        Justification::File { .. } => None,
        Justification::RustFn { path, item } => {
            let source = fs::read_to_string(root.join(path)).ok()?;
            rust_source::function_line(&source, item).ok().flatten()
        }
    }
}
