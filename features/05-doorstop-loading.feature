Feature: Read a Doorstop tree the way Doorstop does

  @justifies:SPEC016 @justifies:SPEC022
  Scenario: L1 No Doorstop tree is an error
    Given an empty project
    When I run "emperor check"
    Then the exit code is 2
    And the error output contains "no Doorstop documents found"

  @justifies:SPEC020
  Scenario: L2 Items in child documents are checked
    Given a Doorstop document "SYS" with no parent
    And a Doorstop document "REQ" with parent "SYS"
    And requirement "REQ001" referencing file "tests/login.rs"
    When I run "emperor check"
    Then the output contains "REQ001: file:tests/login.rs has not been accepted"

  @justifies:SPEC020
  Scenario: L3 An item with two parents is checked once
    Given a Doorstop document "SYS" with no parent
    And a Doorstop document "REQ" with parent "SYS"
    And a Doorstop document "LLR" with parent "REQ"
    And requirement "SYS001" without justifications
    And requirement "REQ001" without justifications
    And requirement "REQ002" without justifications
    And requirement "LLR001" referencing file "tests/login.rs"
    And requirement "LLR001" is linked to "REQ001" and "REQ002"
    When I run "emperor check"
    Then the output contains "LLR001: file:tests/login.rs has not been accepted" exactly once

  @justifies:SPEC020
  Scenario: L4 Folders with .doorstop.skip-all are not checked
    Given a Doorstop document "REQ" with no parent
    And a Doorstop document "TMP" with parent "REQ" in folder "vendor/tmp"
    And the folder "vendor" contains a ".doorstop.skip-all" file
    And requirement "TMP001" referencing file "tests/tmp.rs"
    When I run "emperor check"
    Then the exit code is 0
    And the output does not contain "TMP001"

  @justifies:SPEC021
  Scenario: L5 Link fingerprints written by Doorstop are understood
    # Doorstop writes links as either "- REQ001" or "- REQ001: <fingerprint>".
    Given a Doorstop document "SYS" with no parent
    And a Doorstop document "REQ" with parent "SYS"
    And requirement "SYS001" without justifications
    And the file "req/REQ001.yml" contains:
      """
      active: true
      links:
      - SYS001: M0gAMgVduFlIIzNSLX-o1cn18LAimL2KNsARa9lNWCM=
      text: |
        Linked with a fingerprint
      """
    When I run "emperor check"
    Then the exit code is 0

  @justifies:SPEC022
  Scenario: L6 Markdown items are rejected clearly
    # Doorstop 3 can store items as Markdown with front matter. Not supported yet.
    Given a Doorstop document "REQ" with no parent and item format "markdown"
    When I run "emperor check"
    Then the exit code is 2
    And the error output contains "REQ: Markdown items are not supported"

  @justifies:SPEC022
  Scenario: L7 An invalid item file is an error naming the file
    Given a Doorstop document "REQ" with no parent
    And the file "req/REQ001.yml" contains:
      """
      active: [not, a, bool
      """
    When I run "emperor check"
    Then the exit code is 2
    And the error output contains "req/REQ001.yml"

  @justifies:SPEC023
  Scenario: L8 The project root can be given explicitly
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" referencing file "tests/login.rs"
    When I run "emperor --root <project> check" from another folder
    Then the output contains "REQ001: file:tests/login.rs has not been accepted"

  @justifies:SPEC021
  Scenario: L9 A document that adds justified_by to Doorstop's review stamp loads
    # The configuration recommended in the README.
    Given a Doorstop document "REQ" with no parent
    And the document "REQ" includes "justified_by" in its reviewed attributes
    And requirement "REQ001" referencing file "tests/login.rs"
    And the justifications of "REQ001" have been accepted
    When I run "emperor check"
    Then the exit code is 0
