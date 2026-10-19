//! The request a judge receives: a requirement and the content of its justifications.

use std::path::Path;

use serde::Serialize;

use crate::doorstop::Tree;
use crate::error::{Error, Result};
use crate::evidence::Evidence;
use crate::files::read_source;
use crate::justification::Justification;
use crate::{gherkin_source, rust_source};

/// What the judge is asked to assess.
#[derive(Debug, Serialize)]
pub struct Request {
    requirement: RequirementText,
    justifications: Vec<JustificationContent>,
}

#[derive(Debug, Serialize)]
struct RequirementText {
    id: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct JustificationContent {
    #[serde(flatten)]
    justification: Justification,
    content: String,
}

/// The request for a requirement: its text and the content of every justification.
pub fn request(tree: &Tree, evidence: &Evidence, uid: &str) -> Result<Request> {
    let item = tree
        .items
        .get(uid)
        .ok_or_else(|| Error::UnknownRequirement(uid.to_string()))?;
    let justifications = evidence
        .of(uid)
        .map(|justification| {
            Ok(JustificationContent {
                content: content_of(&tree.root, uid, justification)?,
                justification: justification.clone(),
            })
        })
        .collect::<Result<_>>()?;
    Ok(Request {
        requirement: RequirementText {
            id: uid.to_string(),
            text: item.text.clone(),
        },
        justifications,
    })
}

/// The text of a justification: a whole file, or just the function or scenario.
fn content_of(root: &Path, uid: &str, justification: &Justification) -> Result<String> {
    let not_found = || Error::JustificationNotFound {
        requirement: uid.to_string(),
        justification: justification.clone(),
    };
    let path = root.join(justification.path());
    let source = read_source(&path)?.ok_or_else(not_found)?;
    let content = match justification {
        Justification::File { .. } => Some(source),
        Justification::RustFn { item, .. } => {
            rust_source::function_source(&source, item).map_err(Error::parse(&path))?
        }
        Justification::Gherkin { scenario, .. } => {
            let feature = gherkin_source::parse(&source).map_err(Error::parse(&path))?;
            gherkin_source::scenario_source(&feature, &source, scenario)
        }
    };
    content.ok_or_else(not_found)
}
