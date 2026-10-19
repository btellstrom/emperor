//! Renders the problems `check` found for people and for CI systems.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::justification::Justification;
use crate::problem::Problem;

/// Problems one per line, followed by a summary.
pub fn human(problems: &[Problem]) -> String {
    if problems.is_empty() {
        return String::new();
    }
    let mut text: String = problems
        .iter()
        .map(|problem| format!("{problem}\n"))
        .collect();
    text.push('\n');
    text.push_str(&summary(problems));
    text.push('\n');
    text
}

fn summary(problems: &[Problem]) -> String {
    let requirements: BTreeSet<&str> = problems.iter().filter_map(Problem::requirement).collect();
    let mut summary = counted(problems.len(), "problem");
    if !requirements.is_empty() {
        write!(
            summary,
            " in {}",
            counted(requirements.len(), "requirement")
        )
        .expect("writing to a String");
    }
    summary.push('.');
    let acceptable: BTreeSet<&str> = problems
        .iter()
        .filter(|problem| problem.is_resolved_by_accepting())
        .filter_map(Problem::requirement)
        .collect();
    if !acceptable.is_empty() {
        let ids: Vec<&str> = acceptable.into_iter().collect();
        write!(
            summary,
            " Review the changes, then run `emperor accept {}`.",
            ids.join(" ")
        )
        .expect("writing to a String");
    }
    summary
}

fn counted(count: usize, noun: &str) -> String {
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {noun}{plural}")
}

/// Problems as a JSON document: `{ "problems": [...] }`.
pub fn json(problems: &[Problem]) -> String {
    let document = serde_json::json!({ "problems": problems });
    serde_json::to_string_pretty(&document).expect("problems always serialise to JSON") + "\n"
}

/// Problems as GitHub Actions error annotations.
///
/// `line_of` finds the line a justification starts on, where it has one.
pub fn github(problems: &[Problem], line_of: impl Fn(&Justification) -> Option<usize>) -> String {
    problems
        .iter()
        .map(|problem| annotation(problem, &line_of))
        .collect()
}

fn annotation(problem: &Problem, line_of: &impl Fn(&Justification) -> Option<usize>) -> String {
    let mut properties = Vec::new();
    if let Some(location) = problem.location() {
        properties.push(format!("file={}", escape_property(&location.path)));
        properties.push(format!("line={}", location.line));
    } else if let Some(justification) = problem.justification() {
        properties.push(format!("file={}", escape_property(justification.path())));
        if let Some(line) = line_of(justification) {
            properties.push(format!("line={line}"));
        }
    }
    if let Some(requirement) = problem.requirement() {
        properties.push(format!("title={}", escape_property(requirement)));
    }
    format!(
        "::error {}::{}\n",
        properties.join(","),
        escape_message(&problem.description())
    )
}

/// Escapes annotation text the way GitHub's toolkit does.
fn escape_message(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Escapes an annotation property, which additionally cannot contain `:` or `,`.
fn escape_property(text: &str) -> String {
    escape_message(text).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::file;

    fn changed(uid: &str, path: &str) -> Problem {
        Problem::Changed {
            requirement: uid.into(),
            justification: file(path),
        }
    }

    fn no_lines(_: &Justification) -> Option<usize> {
        None
    }

    #[test]
    fn human_output_lists_problems_and_how_to_accept_them() {
        assert_eq!(
            human(&[changed("REQ001", "a.rs")]),
            "REQ001: file:a.rs changed since it was accepted\n\n\
             1 problem in 1 requirement. Review the changes, then run `emperor accept REQ001`.\n"
        );
    }

    #[test]
    fn human_summary_counts_problems_and_requirements() {
        let problems = [
            changed("REQ001", "a.rs"),
            changed("REQ001", "b.rs"),
            changed("REQ002", "a.rs"),
        ];
        assert!(human(&problems).ends_with(
            "3 problems in 2 requirements. Review the changes, then run `emperor accept REQ001 REQ002`.\n"
        ));
    }

    #[test]
    fn human_summary_only_suggests_accepting_what_accepting_fixes() {
        let problems = [Problem::Missing {
            requirement: "REQ001".into(),
            justification: file("a.rs"),
        }];
        assert!(human(&problems).ends_with("\n1 problem in 1 requirement.\n"));
    }

    #[test]
    fn human_summary_leaves_out_requirements_when_no_problem_is_about_one() {
        let problems = [Problem::InvalidTag {
            location: crate::problem::Location {
                path: "lib.rs".into(),
                line: 1,
            },
            reason: "justifies is only supported on functions".into(),
        }];
        assert!(human(&problems).ends_with("\n1 problem.\n"));
    }

    #[test]
    fn human_output_is_empty_without_problems() {
        assert_eq!(human(&[]), "");
    }

    #[test]
    fn json_describes_each_problem() {
        let rendered: serde_json::Value =
            serde_json::from_str(&json(&[changed("REQ001", "a.rs")])).unwrap();
        assert_eq!(
            rendered,
            serde_json::json!({
                "problems": [{
                    "requirement": "REQ001",
                    "kind": "changed",
                    "justification": { "type": "file", "path": "a.rs" }
                }]
            })
        );
    }

    #[test]
    fn json_names_the_kind_of_every_problem() {
        let problems = [
            Problem::Unjustified {
                requirement: "REQ001".into(),
            },
            Problem::RequirementRemoved {
                requirement: "REQ002".into(),
            },
        ];
        let rendered: serde_json::Value = serde_json::from_str(&json(&problems)).unwrap();
        assert_eq!(rendered["problems"][0]["kind"], "unjustified");
        assert_eq!(rendered["problems"][1]["kind"], "requirement-removed");
    }

    #[test]
    fn github_annotates_the_justification_file() {
        assert_eq!(
            github(&[changed("REQ001", "a.rs")], no_lines),
            "::error file=a.rs,title=REQ001::file:a.rs changed since it was accepted\n"
        );
    }

    #[test]
    fn github_annotates_the_line_when_known() {
        assert_eq!(
            github(&[changed("REQ001", "a.rs")], |_| Some(3)),
            "::error file=a.rs,line=3,title=REQ001::file:a.rs changed since it was accepted\n"
        );
    }

    #[test]
    fn github_annotates_problems_without_a_file_by_title_only() {
        let problems = [Problem::RequirementRemoved {
            requirement: "REQ001".into(),
        }];
        assert_eq!(
            github(&problems, no_lines),
            "::error title=REQ001::requirement no longer exists\n"
        );
    }

    #[test]
    fn github_escapes_special_characters() {
        assert_eq!(
            github(&[changed("REQ001", "a,b:c%.rs")], no_lines),
            "::error file=a%2Cb%3Ac%25.rs,title=REQ001::file:a,b:c%25.rs changed since it was accepted\n"
        );
    }
}
