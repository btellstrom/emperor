Feature: Require justifying tests to have passed

  # A link only says a test is related; test results say it passed.
  # Results are read from JUnit XML (cargo nextest, most other test runners).
  # A rust-fn justification matches a test case by function name and module path.

  Background:
    Given a Doorstop document "REQ" with no parent
    And the file "tests/login.rs" contains:
      """
      #[test]
      fn rejects_bad_password() {}
      """
    And requirement "REQ001" justified by Rust function "rejects_bad_password" in "tests/login.rs"
    And the justifications of "REQ001" have been accepted

  @justifies:SPEC031
  Scenario: TR1 A passing justifying test passes
    Given the JUnit report "junit.xml" says "rejects_bad_password" passed
    When I run "emperor check --test-results junit.xml"
    Then the exit code is 0

  @justifies:SPEC031
  Scenario: TR2 A failing justifying test fails
    Given the JUnit report "junit.xml" says "rejects_bad_password" failed
    When I run "emperor check --test-results junit.xml"
    Then the exit code is 1
    And the output contains "REQ001: test rejects_bad_password failed"

  @justifies:SPEC031
  Scenario: TR3 A justifying test missing from the report fails
    Given the JUnit report "junit.xml" contains no tests
    When I run "emperor check --test-results junit.xml"
    Then the exit code is 1
    And the output contains "REQ001: test rejects_bad_password did not run"

  @justifies:SPEC031
  Scenario: TR4 A skipped justifying test fails
    Given the JUnit report "junit.xml" says "rejects_bad_password" was skipped
    When I run "emperor check --test-results junit.xml"
    Then the exit code is 1
    And the output contains "REQ001: test rejects_bad_password was skipped"

  @justifies:SPEC032
  Scenario: TR5 Without --test-results, test outcomes are not checked
    When I run "emperor check"
    Then the exit code is 0
