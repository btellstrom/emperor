//! Finds `@justifies:<ID>` tags in Gherkin and fingerprints scenarios by their steps.

use std::fmt::Write;

use gherkin::{Feature, GherkinEnv, Rule, Scenario, Step, Table};

use crate::fingerprint::sha256;

/// The tag prefix that names a requirement a scenario justifies.
pub const JUSTIFIES_PREFIX: &str = "justifies:";

/// A scenario naming a requirement it justifies.
#[derive(Debug, PartialEq, Eq)]
pub struct ScenarioTag {
    pub scenario: String,
    pub requirement: String,
    /// The line the tag is on.
    pub line: usize,
}

pub fn parse(source: &str) -> Result<Feature, String> {
    Feature::parse(source, GherkinEnv::default()).map_err(|error| error.to_string())
}

/// Every requirement each scenario names, directly or through its Feature or Rule.
pub fn justifies_tags(feature: &Feature, source: &str) -> Vec<ScenarioTag> {
    let lines: Vec<&str> = source.lines().collect();
    let mut tags = Vec::new();
    for (scenario, rule) in scenarios(feature) {
        let mut tagged = vec![(&feature.tags, feature.position.line)];
        tagged.extend(rule.map(|rule| (&rule.tags, rule.position.line)));
        tagged.push((&scenario.tags, scenario.position.line));
        for (owner_tags, keyword_line) in tagged {
            for requirement in requirements(owner_tags) {
                tags.push(ScenarioTag {
                    scenario: scenario.name.clone(),
                    requirement: requirement.to_string(),
                    line: tag_line(&lines, keyword_line, requirement),
                });
            }
        }
    }
    tags
}

/// The name of a tagged scenario that another scenario in the file shares.
pub fn duplicate_tagged_name(feature: &Feature) -> Option<String> {
    let names: Vec<&str> = scenarios(feature)
        .map(|(scenario, _)| scenario.name.as_str())
        .collect();
    scenarios(feature)
        .filter(|(scenario, rule)| names_requirements(feature, *rule, scenario))
        .map(|(scenario, _)| scenario.name.as_str())
        .find(|name| names.iter().filter(|other| *other == name).count() > 1)
        .map(str::to_string)
}

/// The fingerprint of the named scenario, or `None` when there is no such scenario.
pub fn fingerprint_scenario(feature: &Feature, name: &str) -> Option<String> {
    let (scenario, rule) = find_scenario(feature, name)?;
    let mut text = String::new();
    let backgrounds = [
        feature.background.as_ref(),
        rule.and_then(|rule| rule.background.as_ref()),
    ];
    for background in backgrounds.into_iter().flatten() {
        background
            .steps
            .iter()
            .for_each(|step| write_step(step, &mut text));
    }
    scenario
        .steps
        .iter()
        .for_each(|step| write_step(step, &mut text));
    for examples in &scenario.examples {
        text.push_str("Examples\n");
        examples
            .table
            .iter()
            .for_each(|table| write_table(table, &mut text));
    }
    Some(sha256(text.as_bytes()))
}

/// The line the named scenario starts on.
pub fn scenario_line(feature: &Feature, name: &str) -> Option<usize> {
    find_scenario(feature, name).map(|(scenario, _)| scenario.position.line)
}

/// Every scenario with the Rule it is in, if any.
fn scenarios(feature: &Feature) -> impl Iterator<Item = (&Scenario, Option<&Rule>)> {
    let direct = feature.scenarios.iter().map(|scenario| (scenario, None));
    let in_rules = feature.rules.iter().flat_map(|rule| {
        rule.scenarios
            .iter()
            .map(move |scenario| (scenario, Some(rule)))
    });
    direct.chain(in_rules)
}

fn find_scenario<'a>(feature: &'a Feature, name: &str) -> Option<(&'a Scenario, Option<&'a Rule>)> {
    scenarios(feature).find(|(scenario, _)| scenario.name == name)
}

fn names_requirements(feature: &Feature, rule: Option<&Rule>, scenario: &Scenario) -> bool {
    let rule_tags = rule.map(|rule| rule.tags.as_slice()).unwrap_or_default();
    [&feature.tags[..], rule_tags, &scenario.tags[..]]
        .into_iter()
        .any(|tags| requirements(tags).next().is_some())
}

fn requirements(tags: &[String]) -> impl Iterator<Item = &str> {
    tags.iter()
        .filter_map(|tag| tag.strip_prefix(JUSTIFIES_PREFIX))
}

/// The line of the `@justifies:<requirement>` tag above the keyword on `keyword_line`.
fn tag_line(lines: &[&str], keyword_line: usize, requirement: &str) -> usize {
    let tag = format!("@{JUSTIFIES_PREFIX}{requirement}");
    (1..keyword_line)
        .rev()
        .take_while(|line| is_tag_or_comment(lines[line - 1]))
        .find(|line| lines[line - 1].contains(&tag))
        .unwrap_or(keyword_line)
}

fn is_tag_or_comment(line: &str) -> bool {
    let line = line.trim_start();
    line.is_empty() || line.starts_with('@') || line.starts_with('#')
}

/// A step as text, ignoring whitespace inside it but not its doc string or table.
fn write_step(step: &Step, text: &mut String) {
    let value: Vec<&str> = step.value.split_whitespace().collect();
    writeln!(text, "{} {}", step.keyword.trim(), value.join(" ")).expect("writing to a String");
    if let Some(docstring) = &step.docstring {
        writeln!(text, "\"\"\"\n{docstring}\n\"\"\"").expect("writing to a String");
    }
    step.table.iter().for_each(|table| write_table(table, text));
}

fn write_table(table: &Table, text: &mut String) {
    for row in &table.rows {
        writeln!(text, "|{}|", row.join("|")).expect("writing to a String");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "\
Feature: Login

  Background:
    Given a user \"alice\" with password \"secret\"

  @justifies:REQ001 @slow
  Scenario: A wrong password is rejected
    When \"alice\" logs in with \"wrong\"
    Then the login is rejected

  Scenario: Untagged
    Then nothing

  @justifies:REQ001
  Scenario Outline: Known users
    Given the users:
      | name  | password |
      | alice | secret   |
    When \"<name>\" logs in with:
      \"\"\"
      <password>
      \"\"\"

    Examples:
      | name  |
      | alice |
";

    fn feature(source: &str) -> Feature {
        parse(source).unwrap()
    }

    fn fingerprint(source: &str, name: &str) -> Option<String> {
        fingerprint_scenario(&feature(source), name)
    }

    fn tag(scenario: &str, requirement: &str, line: usize) -> ScenarioTag {
        ScenarioTag {
            scenario: scenario.into(),
            requirement: requirement.into(),
            line,
        }
    }

    #[test]
    fn finds_the_requirements_a_scenario_names() {
        assert_eq!(
            justifies_tags(&feature(SOURCE), SOURCE),
            [
                tag("A wrong password is rejected", "REQ001", 6),
                tag("Known users", "REQ001", 14),
            ]
        );
    }

    #[test]
    fn a_tag_on_the_feature_or_a_rule_applies_to_its_scenarios() {
        let source = "\
@justifies:REQ001
Feature: Login

  Scenario: First
    Then one

  @justifies:REQ002
  Rule: Lockout

    Scenario: Second
      Then two
";
        assert_eq!(
            justifies_tags(&feature(source), source),
            [
                tag("First", "REQ001", 1),
                tag("Second", "REQ001", 1),
                tag("Second", "REQ002", 7),
            ]
        );
    }

    #[test]
    fn a_shared_name_of_a_tagged_scenario_is_found() {
        let source = "Feature: F\n  @justifies:REQ001\n  Scenario: Same\n    Then a\n  Scenario: Same\n    Then b\n";
        assert_eq!(duplicate_tagged_name(&feature(source)), Some("Same".into()));
        assert_eq!(duplicate_tagged_name(&feature(SOURCE)), None);
    }

    #[test]
    fn finds_the_line_a_scenario_starts_on() {
        assert_eq!(scenario_line(&feature(SOURCE), "Untagged"), Some(11));
    }

    #[test]
    fn a_missing_scenario_has_no_fingerprint() {
        assert_eq!(fingerprint(SOURCE, "Missing"), None);
    }

    #[test]
    fn changes_to_steps_change_the_fingerprint() {
        let original = fingerprint(SOURCE, "A wrong password is rejected");
        assert!(original.is_some());
        for (from, to) in [
            ("is rejected", "succeeds"),
            ("password \"secret\"", "password \"other\""),
        ] {
            let changed = SOURCE.replace(from, to);
            assert_ne!(
                fingerprint(&changed, "A wrong password is rejected"),
                original,
                "{from}"
            );
        }
    }

    #[test]
    fn changes_to_tables_doc_strings_and_examples_change_the_fingerprint() {
        let original = fingerprint(SOURCE, "Known users");
        for (from, to) in [
            ("| alice | secret   |", "| alice | hunter2  |"),
            ("      <password>\n", "      <password>!\n"),
            ("      | alice |\n", "      | bob   |\n"),
        ] {
            let changed = SOURCE.replace(from, to);
            assert_ne!(fingerprint(&changed, "Known users"), original, "{from}");
        }
    }

    #[test]
    fn comments_whitespace_and_tags_do_not_change_the_fingerprint() {
        let original = fingerprint(SOURCE, "A wrong password is rejected");
        let changed = SOURCE
            .replace(
                "@justifies:REQ001 @slow",
                "# A comment\n  @justifies:REQ001 @justifies:REQ002",
            )
            .replace("When \"alice\"", "When   \"alice\"   ");
        assert_eq!(
            fingerprint(&changed, "A wrong password is rejected"),
            original
        );
    }

    #[test]
    fn changing_another_scenario_does_not_change_the_fingerprint() {
        let original = fingerprint(SOURCE, "A wrong password is rejected");
        let changed = SOURCE.replace("Then nothing", "Then something");
        assert_eq!(
            fingerprint(&changed, "A wrong password is rejected"),
            original
        );
    }
}
