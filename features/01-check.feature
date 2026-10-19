Feature: Detect changed justifications

  Background:
    Given a Doorstop document "REQ" with no parent

  @justifies:SPEC001 @justifies:SPEC016
  Scenario: C1 An unchanged file justification passes
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    When I run "emperor check"
    Then the exit code is 0

  @pending @justifies:SPEC001 @justifies:SPEC016
  Scenario: C2 A changed file justification fails
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    When "tests/login.rs" is modified
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: file:tests/login.rs changed since it was accepted"

  @pending @justifies:SPEC002
  Scenario: C3 A justification that was never accepted fails
    Given requirement "REQ001" referencing file "tests/login.rs"
    When I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: file:tests/login.rs has not been accepted"

  @pending @justifies:SPEC003
  Scenario: C4 A deleted file justification fails
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    When "tests/login.rs" is deleted
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: file:tests/login.rs not found"

  @pending @justifies:SPEC001
  Scenario: C5 A whitespace-only change to a file justification fails
    # File justifications are compared byte for byte.
    Given requirement "REQ001" referencing file "docs/threat-model.md"
    And the justifications of "REQ001" have been accepted
    When a trailing newline is appended to "docs/threat-model.md"
    And I run "emperor check"
    Then the exit code is 1

  @pending @justifies:SPEC005
  Scenario: C6 Inactive requirements are not checked
    Given requirement "REQ001" referencing file "tests/login.rs"
    And requirement "REQ001" is inactive
    When I run "emperor check"
    Then the exit code is 0

  @pending @justifies:SPEC006
  Scenario: C7 A requirement without justifications passes by default
    Given requirement "REQ001" without justifications
    When I run "emperor check"
    Then the exit code is 0

  @pending @justifies:SPEC006
  Scenario: C8 A requirement without justifications fails when justifications are required
    Given requirement "REQ001" without justifications
    When I run "emperor check --require-justification"
    Then the exit code is 1
    And the output contains "REQ001: has no justifications"

  @pending @justifies:SPEC007
  Scenario: C9 All problems are reported, not just the first
    Given requirement "REQ001" referencing file "tests/login.rs"
    And requirement "REQ002" referencing file "tests/logout.rs"
    When I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: file:tests/login.rs has not been accepted"
    And the output contains "REQ002: file:tests/logout.rs has not been accepted"

  @pending @justifies:SPEC001
  Scenario: C10 A file shared by two requirements is reported for both
    Given requirement "REQ001" referencing file "tests/auth.rs"
    And requirement "REQ002" referencing file "tests/auth.rs"
    And the justifications of "REQ001" have been accepted
    And the justifications of "REQ002" have been accepted
    When "tests/auth.rs" is modified
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: file:tests/auth.rs changed since it was accepted"
    And the output contains "REQ002: file:tests/auth.rs changed since it was accepted"

  @pending @justifies:SPEC004
  Scenario: C11 A deleted requirement with accepted justifications fails until accepted
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    When requirement "REQ001" is deleted
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: requirement no longer exists"

  @pending @justifies:SPEC008
  Scenario: C12 Check never modifies the lockfile
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    When "tests/login.rs" is modified
    And I run "emperor check"
    Then the lockfile is unchanged
