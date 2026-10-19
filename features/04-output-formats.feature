Feature: Machine-readable check output for CI

  Background:
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    And "tests/login.rs" is modified

  @pending @justifies:SPEC017
  Scenario: O1 Human-readable output is the default
    When I run "emperor check"
    Then the output is:
      """
      REQ001: file:tests/login.rs changed since it was accepted

      1 problem in 1 requirement. Review the changes, then run `emperor accept REQ001`.
      """

  @pending @justifies:SPEC018
  Scenario: O2 JSON output
    When I run "emperor check --format json"
    Then the exit code is 1
    And the output is JSON equal to:
      """
      {
        "problems": [
          {
            "requirement": "REQ001",
            "kind": "changed",
            "justification": { "type": "file", "path": "tests/login.rs" }
          }
        ]
      }
      """

  @pending @justifies:SPEC018
  Scenario: O3 JSON output with no problems
    Given the justifications of "REQ001" have been accepted
    When I run "emperor check --format json"
    Then the exit code is 0
    And the output is JSON equal to:
      """
      { "problems": [] }
      """

  @pending @justifies:SPEC019
  Scenario: O4 GitHub Actions annotations
    # Shows up as an error on the changed file in the pull request.
    When I run "emperor check --format github"
    Then the exit code is 1
    And the output contains "::error file=tests/login.rs,title=REQ001::file:tests/login.rs changed since it was accepted"

  @pending @justifies:SPEC019
  Scenario: O5 Rust function annotations point at the function's line
    Given the file "tests/auth.rs" contains:
      """
      // helpers

      #[test]
      fn hashes_password() {}
      """
    And requirement "REQ002" justified by Rust function "hashes_password" in "tests/auth.rs"
    And the justifications of "REQ002" have been accepted
    And in "tests/auth.rs" "{}" is replaced with "{ assert!(true) }"
    When I run "emperor check --format github"
    Then the output contains "::error file=tests/auth.rs,line=3,title=REQ002::"
