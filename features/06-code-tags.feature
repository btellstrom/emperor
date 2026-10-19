Feature: Link Rust functions to requirements with the justifies attribute

  # `#[justifies("REQ001", ...)]` from the `emperor-tags` crate marks a function as a
  # justification. The macro returns the function unchanged; emperor finds the attribute
  # by parsing the source. The link lives in the code instead of the Doorstop item.
  # The `justifies` attribute itself is not part of the function's fingerprint.

  Background:
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" without justifications
    And requirement "REQ002" without justifications

  @pending @justifies:SPEC024
  Scenario: G1 A tagged function becomes a justification
    Given the file "tests/login.rs" contains:
      """
      #[test]
      #[justifies("REQ001")]
      fn rejects_bad_password() {}
      """
    When I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: rust-fn:tests/login.rs::rejects_bad_password has not been accepted"

  @pending @justifies:SPEC024
  Scenario: G2 A tagged function is tracked like any other justification
    Given the file "tests/login.rs" contains:
      """
      #[test]
      #[justifies("REQ001")]
      fn rejects_bad_password() {}
      """
    And the justifications of "REQ001" have been accepted
    When in "tests/login.rs" "{}" is replaced with "{ assert!(true) }"
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: rust-fn:tests/login.rs::rejects_bad_password changed since it was accepted"

  @pending @justifies:SPEC024
  Scenario: G3 One attribute can justify several requirements
    Given the file "tests/login.rs" contains:
      """
      #[test]
      #[justifies("REQ001", "REQ002")]
      fn rejects_bad_password() {}
      """
    When I run "emperor check"
    Then the output contains "REQ001: rust-fn:tests/login.rs::rejects_bad_password has not been accepted"
    And the output contains "REQ002: rust-fn:tests/login.rs::rejects_bad_password has not been accepted"

  @pending @justifies:SPEC028
  Scenario: G4 Adding a requirement to the attribute does not invalidate the others
    Given the file "tests/login.rs" contains:
      """
      #[test]
      #[justifies("REQ001")]
      fn rejects_bad_password() {}
      """
    And the justifications of "REQ001" have been accepted
    When in "tests/login.rs" "#[justifies(\"REQ001\")]" is replaced with "#[justifies(\"REQ001\", \"REQ002\")]"
    And I run "emperor check"
    Then the output does not contain "REQ001"
    And the output contains "REQ002: rust-fn:tests/login.rs::rejects_bad_password has not been accepted"

  @pending @justifies:SPEC029
  Scenario: G5 An attribute naming an unknown requirement fails
    Given the file "tests/login.rs" contains:
      """
      #[test]
      #[justifies("REQ999")]
      fn rejects_bad_password() {}
      """
    When I run "emperor check"
    Then the exit code is 1
    And the output contains "tests/login.rs:2: unknown requirement REQ999"

  @pending @justifies:SPEC024
  Scenario: G6 The fully qualified attribute path is recognised
    Given the file "tests/login.rs" contains:
      """
      #[test]
      #[emperor_tags::justifies("REQ001")]
      fn rejects_bad_password() {}
      """
    When I run "emperor check"
    Then the output contains "REQ001: rust-fn:tests/login.rs::rejects_bad_password has not been accepted"

  @pending @justifies:SPEC014
  Scenario: G7 A tagged function inside a module is addressed by its module path
    Given the file "src/auth.rs" contains:
      """
      #[cfg(test)]
      mod tests {
          #[test]
          #[justifies("REQ001")]
          fn hashes_password() {}
      }
      """
    When I run "emperor check"
    Then the output contains "REQ001: rust-fn:src/auth.rs::tests::hashes_password has not been accepted"

  @pending @justifies:SPEC015
  Scenario: G8 The attribute on something other than a function fails
    # Only functions are supported for now.
    Given the file "src/auth.rs" contains:
      """
      #[justifies("REQ001")]
      struct PasswordPolicy;
      """
    When I run "emperor check"
    Then the exit code is 1
    And the output contains "src/auth.rs:1: justifies is only supported on functions"

  @pending @justifies:SPEC024
  Scenario: G9 A function both tagged and listed in justified_by is reported once
    Given the file "tests/login.rs" contains:
      """
      #[test]
      #[justifies("REQ001")]
      fn rejects_bad_password() {}
      """
    And requirement "REQ001" is also justified by Rust function "rejects_bad_password" in "tests/login.rs"
    When I run "emperor check"
    Then the output contains "REQ001: rust-fn:tests/login.rs::rejects_bad_password has not been accepted" exactly once

  @pending @justifies:SPEC030
  Scenario: G10 Ignored files are not scanned
    Given the file ".gitignore" contains:
      """
      /generated
      """
    And the file "generated/login.rs" contains:
      """
      #[justifies("REQ999")]
      fn generated() {}
      """
    When I run "emperor check"
    Then the exit code is 0
