//! Reads test outcomes from JUnit XML, as written by `cargo nextest` and most test runners.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    Passed,
    Skipped,
    Failed,
}

#[derive(Debug, Default)]
pub struct TestResults {
    /// Test case names with their outcomes.
    cases: Vec<(String, Outcome)>,
}

impl TestResults {
    pub fn from_junit(xml: &str) -> Result<TestResults, String> {
        let document = roxmltree::Document::parse(xml).map_err(|error| error.to_string())?;
        let cases = document
            .descendants()
            .filter(|node| node.has_tag_name("testcase"))
            .map(|testcase| {
                let name = testcase.attribute("name").unwrap_or_default().to_string();
                (name, outcome_of(testcase))
            })
            .collect();
        Ok(TestResults { cases })
    }

    /// The outcome of the test at a module path such as `tests::hashes_password`.
    ///
    /// Runners name tests by their path in the crate, so the name only has to end with it.
    /// When several tests match, the worst outcome counts.
    pub fn outcome(&self, test: &str) -> Option<Outcome> {
        let suffix = format!("::{test}");
        self.cases
            .iter()
            .filter(|(name, _)| name == test || name.ends_with(&suffix))
            .map(|(_, outcome)| *outcome)
            .max()
    }
}

fn outcome_of(testcase: roxmltree::Node) -> Outcome {
    let has_child = |tag: &str| testcase.children().any(|child| child.has_tag_name(tag));
    if has_child("failure") || has_child("error") {
        Outcome::Failed
    } else if has_child("skipped") {
        Outcome::Skipped
    } else {
        Outcome::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results(testcases: &str) -> TestResults {
        let xml = format!(
            "<testsuites><testsuite name=\"crate::login\">{testcases}</testsuite></testsuites>"
        );
        TestResults::from_junit(&xml).unwrap()
    }

    #[test]
    fn reads_passed_failed_and_skipped_tests() {
        let results = results(
            "<testcase name=\"passes\"/>\
             <testcase name=\"fails\"><failure message=\"boom\"/></testcase>\
             <testcase name=\"errors\"><error message=\"panic\"/></testcase>\
             <testcase name=\"skips\"><skipped/></testcase>",
        );
        assert_eq!(results.outcome("passes"), Some(Outcome::Passed));
        assert_eq!(results.outcome("fails"), Some(Outcome::Failed));
        assert_eq!(results.outcome("errors"), Some(Outcome::Failed));
        assert_eq!(results.outcome("skips"), Some(Outcome::Skipped));
    }

    #[test]
    fn a_test_that_is_not_in_the_report_has_no_outcome() {
        assert_eq!(
            results("<testcase name=\"other\"/>").outcome("missing"),
            None
        );
    }

    #[test]
    fn matches_a_test_by_the_end_of_its_module_path() {
        let results = results("<testcase name=\"auth::tests::hashes_password\"/>");
        assert_eq!(
            results.outcome("tests::hashes_password"),
            Some(Outcome::Passed)
        );
        assert_eq!(results.outcome("hashes_password"), Some(Outcome::Passed));
        assert_eq!(results.outcome("password"), None);
    }

    #[test]
    fn the_worst_outcome_of_matching_tests_counts() {
        let results = results(
            "<testcase name=\"a::login\"/><testcase name=\"b::login\"><failure/></testcase>",
        );
        assert_eq!(results.outcome("login"), Some(Outcome::Failed));
    }

    #[test]
    fn invalid_xml_is_an_error() {
        assert!(TestResults::from_junit("<testsuites>").is_err());
    }
}
