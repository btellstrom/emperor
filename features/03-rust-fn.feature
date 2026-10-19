Feature: Justify a requirement with a single Rust function

  # Uses the custom Doorstop attribute `justified_by` with `type: rust-fn`.
  # The function's tokens are fingerprinted, so formatting and comments do not count.

  Background:
    Given a Doorstop document "REQ" with no parent
    And the file "tests/login.rs" contains:
      """
      #[test]
      fn rejects_bad_password() {
          assert!(!login("alice", "wrong"));
      }

      #[test]
      fn accepts_good_password() {
          assert!(login("alice", "secret"));
      }
      """
    And requirement "REQ001" justified by Rust function "rejects_bad_password" in "tests/login.rs"
    And the justifications of "REQ001" have been accepted

  @justifies:SPEC013
  Scenario: R1 An unchanged function passes
    When I run "emperor check"
    Then the exit code is 0

  @justifies:SPEC013
  Scenario: R2 Changing the function body fails
    When in "tests/login.rs" "wrong" is replaced with "Wrong"
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: rust-fn:tests/login.rs::rejects_bad_password changed since it was accepted"

  @justifies:SPEC013
  Scenario: R3 Changing a different function in the same file passes
    When in "tests/login.rs" "secret" is replaced with "hunter2"
    And I run "emperor check"
    Then the exit code is 0

  @justifies:SPEC013
  Scenario: R4 Reformatting the function passes
    When "tests/login.rs" is reformatted with all whitespace inside "rejects_bad_password" doubled
    And I run "emperor check"
    Then the exit code is 0

  @justifies:SPEC013
  Scenario: R5 Changing comments in the function passes
    # Includes doc comments (`///`), even though Rust treats those as attributes.
    When the comment "// a wrong password must fail" is added inside "rejects_bad_password" in "tests/login.rs"
    And I run "emperor check"
    Then the exit code is 0

  @justifies:SPEC013
  Scenario: R6 Changing the function's attributes fails
    # e.g. adding #[ignore] would silently stop the test from running.
    # Exception: the #[justifies(...)] attribute is not fingerprinted (see 06-code-tags).
    When in "tests/login.rs" "#[test]\nfn rejects_bad_password" is replaced with "#[test]\n#[ignore]\nfn rejects_bad_password"
    And I run "emperor check"
    Then the exit code is 1

  @justifies:SPEC003
  Scenario: R7 A renamed function is reported as not found
    When in "tests/login.rs" "fn rejects_bad_password" is replaced with "fn rejects_wrong_password"
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: rust-fn:tests/login.rs::rejects_bad_password not found"

  @justifies:SPEC014
  Scenario: R8 A function inside a module is addressed by its module path
    Given the file "src/auth.rs" contains:
      """
      #[cfg(test)]
      mod tests {
          #[test]
          fn hashes_password() {}
      }
      """
    And requirement "REQ002" justified by Rust function "tests::hashes_password" in "src/auth.rs"
    When I run "emperor accept REQ002"
    Then the exit code is 0
    And running "emperor check" exits with 0

  @justifies:SPEC015
  Scenario: R9 A file that does not parse as Rust is an error
    When the file "tests/login.rs" contains:
      """
      fn rejects_bad_password( {
      """
    And I run "emperor check"
    Then the exit code is 2
    And the error output contains "tests/login.rs"

  @justifies:SPEC015
  Scenario: R10 An unknown justification type is an error
    Given requirement "REQ003" justified by "python-fn" "test_login" in "tests/test_login.py"
    When I run "emperor check"
    Then the exit code is 2
    And the error output contains "REQ003: unknown justification type python-fn"
