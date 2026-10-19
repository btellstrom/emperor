//! Gathers every requirement's justifications: those its Doorstop item names and
//! those that code names with tags.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::doorstop::Tree;
use crate::error::{Error, Result, read_to_string};
use crate::gherkin_source::{self, ScenarioTag};
use crate::justification::{self, Justification};
use crate::problem::{Location, Problem};
use crate::rust_tags::{self, JUSTIFIES, Tag, TagKind};

const MISPLACED_TAG: &str = "justifies is only supported on functions";
const INVALID_TAG_ARGUMENTS: &str = "justifies expects requirement IDs as string literals";

#[derive(Debug, Default)]
pub struct Evidence {
    justifications: BTreeMap<String, BTreeSet<Justification>>,
    /// Tags that name unknown requirements or cannot be used.
    pub problems: Vec<Problem>,
}

impl Evidence {
    /// The justifications of a requirement, in a stable order and without duplicates.
    pub fn of(&self, uid: &str) -> impl Iterator<Item = &Justification> {
        self.justifications.get(uid).into_iter().flatten()
    }

    fn add(&mut self, uid: &str, justification: Justification) {
        self.justifications
            .entry(uid.to_string())
            .or_default()
            .insert(justification);
    }

    fn add_tag(&mut self, tree: &Tree, path: &str, tag: Tag) {
        let location = Location {
            path: path.to_string(),
            line: tag.line,
        };
        match tag.kind {
            TagKind::Justifies {
                function,
                requirements,
            } => self.add_tagged_function(tree, location, &function, requirements),
            TagKind::Misplaced => self.add_invalid_tag(location, MISPLACED_TAG),
            TagKind::InvalidArguments => self.add_invalid_tag(location, INVALID_TAG_ARGUMENTS),
        }
    }

    fn add_tagged_function(
        &mut self,
        tree: &Tree,
        location: Location,
        function: &str,
        requirements: Vec<String>,
    ) {
        for uid in requirements {
            if tree.items.contains_key(&uid) {
                let justification = Justification::RustFn {
                    path: location.path.clone(),
                    item: function.to_string(),
                };
                self.add(&uid, justification);
            } else {
                self.problems.push(Problem::UnknownRequirement {
                    requirement: uid,
                    location: location.clone(),
                });
            }
        }
    }

    fn add_scenario_tag(&mut self, tree: &Tree, path: &str, tag: ScenarioTag) {
        if tree.items.contains_key(&tag.requirement) {
            let justification = Justification::Gherkin {
                path: path.to_string(),
                scenario: tag.scenario,
            };
            self.add(&tag.requirement, justification);
        } else {
            self.problems.push(Problem::UnknownRequirement {
                requirement: tag.requirement,
                location: Location {
                    path: path.to_string(),
                    line: tag.line,
                },
            });
        }
    }

    fn add_invalid_tag(&mut self, location: Location, reason: &str) {
        self.problems.push(Problem::InvalidTag {
            location,
            reason: reason.to_string(),
        });
    }
}

pub fn gather(tree: &Tree) -> Result<Evidence> {
    let mut evidence = Evidence::default();
    for item in tree.items.values() {
        for justification in justification::of_item(item)? {
            evidence.add(&item.uid, justification);
        }
    }
    for path in project_files(&tree.root, "rs")? {
        let source = read_to_string(&path)?;
        // Files that cannot contain a tag are not parsed, so broken test fixtures are no error.
        if !source.contains(JUSTIFIES) {
            continue;
        }
        let tags = rust_tags::justifies_tags(&source).map_err(Error::parse(&path))?;
        let relative = relative_path(&tree.root, &path);
        for tag in tags {
            evidence.add_tag(tree, &relative, tag);
        }
    }
    for path in project_files(&tree.root, "feature")? {
        let source = read_to_string(&path)?;
        let feature = gherkin_source::parse(&source).map_err(Error::parse(&path))?;
        if let Some(name) = gherkin_source::duplicate_tagged_name(&feature) {
            return Err(Error::DuplicateScenario { path, name });
        }
        let relative = relative_path(&tree.root, &path);
        for tag in gherkin_source::justifies_tags(&feature, &source) {
            evidence.add_scenario_tag(tree, &relative, tag);
        }
    }
    Ok(evidence)
}

/// The files with an extension in the project, except those git ignores, in path order.
fn project_files(root: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkBuilder::new(root).require_git(false).build() {
        let entry = entry.map_err(|error| Error::io(root)(io::Error::other(error)))?;
        let path = entry.into_path();
        if path.is_file() && path.extension().is_some_and(|found| found == extension) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// `path` relative to `root`, with `/` separators on every platform.
fn relative_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<_> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect();
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Project, file};

    fn rust_fn(path: &str, item: &str) -> Justification {
        Justification::RustFn {
            path: path.into(),
            item: item.into(),
        }
    }

    fn justifications(evidence: &Evidence, uid: &str) -> Vec<Justification> {
        evidence.of(uid).cloned().collect()
    }

    fn location(path: &str, line: usize) -> Location {
        Location {
            path: path.into(),
            line,
        }
    }

    #[test]
    fn includes_the_justifications_named_in_doorstop() {
        let project = Project::new().requirement("REQ001", &["a.md"]);
        let evidence = gather(&project.tree()).unwrap();
        assert_eq!(justifications(&evidence, "REQ001"), [file("a.md")]);
    }

    #[test]
    fn a_tagged_function_justifies_the_requirements_it_names() {
        let project = Project::new()
            .requirement("REQ001", &[])
            .requirement("REQ002", &[])
            .file(
                "tests/login.rs",
                "#[justifies(\"REQ001\", \"REQ002\")]\nfn login() {}\n",
            );
        let evidence = gather(&project.tree()).unwrap();
        let login = rust_fn("tests/login.rs", "login");
        assert_eq!(justifications(&evidence, "REQ001"), [login]);
        assert_eq!(
            justifications(&evidence, "REQ002"),
            justifications(&evidence, "REQ001")
        );
        assert_eq!(evidence.problems, []);
    }

    #[test]
    fn a_justification_named_twice_is_listed_once() {
        let mut project = Project::new()
            .requirement("REQ001", &[])
            .file("lib.rs", "#[justifies(\"REQ001\")]\nfn login() {}\n");
        project.justify_by_rust_fn("REQ001", "lib.rs", "login");
        let evidence = gather(&project.tree()).unwrap();
        assert_eq!(
            justifications(&evidence, "REQ001"),
            [rust_fn("lib.rs", "login")]
        );
    }

    #[test]
    fn a_tag_naming_an_unknown_requirement_is_a_problem() {
        let project = Project::new()
            .requirement("REQ001", &[])
            .file("lib.rs", "\n#[justifies(\"REQ999\")]\nfn login() {}\n");
        let evidence = gather(&project.tree()).unwrap();
        assert_eq!(
            evidence.problems,
            [Problem::UnknownRequirement {
                requirement: "REQ999".into(),
                location: location("lib.rs", 2)
            }]
        );
    }

    #[test]
    fn misplaced_and_invalid_tags_are_problems() {
        let project = Project::new().requirement("REQ001", &[]).file(
            "lib.rs",
            "#[justifies(\"REQ001\")]\nstruct Policy;\n#[justifies(REQ001)]\nfn login() {}\n",
        );
        let evidence = gather(&project.tree()).unwrap();
        assert_eq!(
            evidence.problems,
            [
                Problem::InvalidTag {
                    location: location("lib.rs", 1),
                    reason: "justifies is only supported on functions".into()
                },
                Problem::InvalidTag {
                    location: location("lib.rs", 3),
                    reason: "justifies expects requirement IDs as string literals".into()
                }
            ]
        );
    }

    const LOGIN_FEATURE: &str = "Feature: Login\n\n  @justifies:REQ001\n  Scenario: Rejected\n    Then the login is rejected\n";

    #[test]
    fn a_tagged_scenario_justifies_the_requirement_it_names() {
        let project = Project::new()
            .requirement("REQ001", &[])
            .file("features/login.feature", LOGIN_FEATURE);
        let evidence = gather(&project.tree()).unwrap();
        assert_eq!(
            justifications(&evidence, "REQ001"),
            [Justification::Gherkin {
                path: "features/login.feature".into(),
                scenario: "Rejected".into()
            }]
        );
    }

    #[test]
    fn a_scenario_tag_naming_an_unknown_requirement_is_a_problem() {
        let project = Project::new()
            .requirement("REQ002", &[])
            .file("features/login.feature", LOGIN_FEATURE);
        assert_eq!(
            gather(&project.tree()).unwrap().problems,
            [Problem::UnknownRequirement {
                requirement: "REQ001".into(),
                location: location("features/login.feature", 3)
            }]
        );
    }

    #[test]
    fn a_tagged_scenario_sharing_its_name_is_an_error() {
        let duplicated = format!("{LOGIN_FEATURE}  Scenario: Rejected\n    Then again\n");
        let project = Project::new()
            .requirement("REQ001", &[])
            .file("features/login.feature", &duplicated);
        let error = gather(&project.tree()).unwrap_err();
        assert!(matches!(error, Error::DuplicateScenario { name, .. } if name == "Rejected"));
    }

    #[test]
    fn a_feature_file_that_does_not_parse_is_an_error() {
        let project = Project::new().requirement("REQ001", &[]).file(
            "features/broken.feature",
            "Feature: Login\n  Scenario without a colon\n",
        );
        let error = gather(&project.tree()).unwrap_err();
        assert!(matches!(error, Error::Parse { .. }));
    }

    #[test]
    fn ignored_files_are_not_scanned() {
        let project = Project::new()
            .requirement("REQ001", &[])
            .file(".gitignore", "/generated\n")
            .file(
                "generated/lib.rs",
                "#[justifies(\"REQ999\")]\nfn login() {}\n",
            );
        assert_eq!(gather(&project.tree()).unwrap().problems, []);
    }

    #[test]
    fn rust_files_that_do_not_mention_the_attribute_are_not_parsed() {
        let project = Project::new()
            .requirement("REQ001", &[])
            .file("tests/ui/broken.rs", "fn broken( {\n");
        assert!(gather(&project.tree()).is_ok());
    }

    #[test]
    fn an_unparsable_file_that_mentions_the_attribute_is_an_error() {
        let project = Project::new()
            .requirement("REQ001", &[])
            .file("lib.rs", "#[justifies(\"REQ001\")]\nfn broken( {\n");
        let error = gather(&project.tree()).unwrap_err();
        assert!(matches!(error, Error::Parse { .. }));
    }
}
