Feature: Accept reviewed justifications

  Background:
    Given a Doorstop document "REQ" with no parent

  @pending @justifies:SPEC009
  Scenario: A1 Accepting a changed justification makes check pass again
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    And "tests/login.rs" is modified
    When I run "emperor accept REQ001"
    Then the exit code is 0
    And running "emperor check" exits with 0

  @pending @justifies:SPEC009
  Scenario: A2 Accepting one requirement leaves the others untouched
    Given requirement "REQ001" referencing file "tests/login.rs"
    And requirement "REQ002" referencing file "tests/logout.rs"
    When I run "emperor accept REQ001"
    Then the lockfile contains an entry for "REQ001"
    And the lockfile does not contain an entry for "REQ002"

  @pending @justifies:SPEC009
  Scenario: A3 Accept all requirements at once
    Given requirement "REQ001" referencing file "tests/login.rs"
    And requirement "REQ002" referencing file "tests/logout.rs"
    When I run "emperor accept --all"
    Then the exit code is 0
    And running "emperor check" exits with 0

  @pending @justifies:SPEC011
  Scenario: A4 Accepting an unknown requirement is an error
    Given requirement "REQ001" referencing file "tests/login.rs"
    When I run "emperor accept REQ999"
    Then the exit code is 2
    And the error output contains "unknown requirement REQ999"
    And no lockfile exists

  @pending @justifies:SPEC011
  Scenario: A5 Accepting a requirement whose justification is missing is an error
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    And "tests/login.rs" is deleted
    When I run "emperor accept REQ001"
    Then the exit code is 2
    And the error output contains "REQ001: file:tests/login.rs not found"
    And the lockfile is unchanged

  @pending @justifies:SPEC011
  Scenario: A6 Accept requires requirement IDs or --all
    Given requirement "REQ001" referencing file "tests/login.rs"
    When I run "emperor accept"
    Then the exit code is 2

  @pending @justifies:SPEC010
  Scenario: A7 Accepting a requirement forgets justifications it no longer references
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    And the reference to "tests/login.rs" is removed from "REQ001"
    When I run "emperor accept REQ001"
    Then running "emperor check" exits with 0

  @pending @justifies:SPEC010
  Scenario: A8 Accepting a deleted requirement removes it from the lockfile
    Given requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    And requirement "REQ001" is deleted
    When I run "emperor accept REQ001"
    Then the exit code is 0
    And the lockfile does not contain an entry for "REQ001"

  @pending @justifies:SPEC012
  Scenario: A9 The lockfile is deterministic
    # Byte-identical output keeps lockfile diffs in pull requests minimal.
    Given requirement "REQ002" referencing file "tests/logout.rs"
    And requirement "REQ001" referencing file "tests/login.rs"
    When I run "emperor accept --all"
    And I run "emperor accept --all"
    Then the lockfile is unchanged since the first accept
    And "REQ001" appears before "REQ002" in the lockfile
