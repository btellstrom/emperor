Feature: Link Gherkin scenarios to requirements with tags

  # A scenario tagged `@justifies:<ID>` is a justification of that requirement.
  # A tag on the Feature applies to every scenario in it.
  # A scenario's fingerprint covers its steps, the Background steps, doc strings,
  # data tables and Examples. Comments, whitespace and tags are not part of it.
  # All `*.feature` files under the project root are scanned, except ignored files.

  Background:
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" without justifications
    And requirement "REQ002" without justifications

  @justifies:SPEC025
  Scenario: H1 A tagged scenario becomes a justification
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          When "alice" logs in with "wrong"
          Then the login is rejected
      """
    When I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: gherkin:features/login.feature::A wrong password is rejected has not been accepted"

  @justifies:SPEC026
  Scenario: H2 Changing a step fails
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          When "alice" logs in with "wrong"
          Then the login is rejected
      """
    And the justifications of "REQ001" have been accepted
    When in "features/login.feature" "Then the login is rejected" is replaced with "Then the login succeeds"
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: gherkin:features/login.feature::A wrong password is rejected changed since it was accepted"

  @justifies:SPEC026
  Scenario: H3 Changing a doc string or data table fails
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: Known users
          Given the users:
            | name  | password |
            | alice | secret   |
      """
    And the justifications of "REQ001" have been accepted
    When in "features/login.feature" "secret" is replaced with "hunter2"
    And I run "emperor check"
    Then the exit code is 1

  @justifies:SPEC026
  Scenario: H4 Changing the Background fails
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        Background:
          Given a user "alice" with password "secret"

        @justifies:REQ001
        Scenario: A wrong password is rejected
          When "alice" logs in with "wrong"
          Then the login is rejected
      """
    And the justifications of "REQ001" have been accepted
    When in "features/login.feature" "with password \"secret\"" is replaced with "with password \"wrong\""
    And I run "emperor check"
    Then the exit code is 1

  @justifies:SPEC026
  Scenario: H5 Changing the Examples of a Scenario Outline fails
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario Outline: Wrong passwords are rejected
          When "alice" logs in with "<password>"
          Then the login is rejected

          Examples:
            | password |
            | wrong    |
      """
    And the justifications of "REQ001" have been accepted
    When in "features/login.feature" "| wrong    |" is replaced with "| WRONG    |"
    And I run "emperor check"
    Then the exit code is 1

  @justifies:SPEC026
  Scenario: H6 Comments, whitespace and other tags do not count
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          When "alice" logs in with "wrong"
          Then the login is rejected
      """
    And the justifications of "REQ001" have been accepted
    When the file "features/login.feature" contains:
      """
      Feature: Login

        # The most common attack.
        @justifies:REQ001 @slow
        Scenario: A wrong password is rejected
          When   "alice" logs in with "wrong"
          Then the login is rejected
      """
    And I run "emperor check"
    Then the exit code is 0

  @justifies:SPEC028
  Scenario: H7 Adding a requirement tag does not invalidate the others
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          When "alice" logs in with "wrong"
          Then the login is rejected
      """
    And the justifications of "REQ001" have been accepted
    When in "features/login.feature" "@justifies:REQ001" is replaced with "@justifies:REQ001 @justifies:REQ002"
    And I run "emperor check"
    Then the output does not contain "REQ001"
    And the output contains "REQ002: gherkin:features/login.feature::A wrong password is rejected has not been accepted"

  @justifies:SPEC025
  Scenario: H8 A tag on the Feature applies to every scenario
    Given the file "features/login.feature" contains:
      """
      @justifies:REQ001
      Feature: Login

        Scenario: A wrong password is rejected
          Then the login is rejected

        Scenario: A right password is accepted
          Then the login succeeds
      """
    When I run "emperor check"
    Then the output contains "REQ001: gherkin:features/login.feature::A wrong password is rejected has not been accepted"
    And the output contains "REQ001: gherkin:features/login.feature::A right password is accepted has not been accepted"

  @justifies:SPEC029
  Scenario: H9 A tag naming an unknown requirement fails
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ999
        Scenario: A wrong password is rejected
          Then the login is rejected
      """
    When I run "emperor check"
    Then the exit code is 1
    And the output contains "features/login.feature:3: unknown requirement REQ999"

  @justifies:SPEC027
  Scenario: H10 A renamed scenario is reported as not found
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          Then the login is rejected
      """
    And the justifications of "REQ001" have been accepted
    When in "features/login.feature" "A wrong password is rejected" is replaced with "Wrong passwords are rejected"
    And I run "emperor check"
    Then the exit code is 1
    And the output contains "REQ001: gherkin:features/login.feature::A wrong password is rejected not found"
    And the output contains "REQ001: gherkin:features/login.feature::Wrong passwords are rejected has not been accepted"

  @justifies:SPEC027
  Scenario: H11 Two scenarios with the same name in one file is an error
    # The name identifies the scenario, so it must be unique within its file.
    Given the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: Rejected
          Then the login is rejected

        Scenario: Rejected
          Then the login is rejected
      """
    When I run "emperor check"
    Then the exit code is 2
    And the error output contains "features/login.feature: duplicate scenario name \"Rejected\""

  @justifies:SPEC015
  Scenario: H12 A feature file that does not parse is an error
    Given the file "features/login.feature" contains:
      """
      Feature: Login
        Scenario without a colon
      """
    When I run "emperor check"
    Then the exit code is 2
    And the error output contains "features/login.feature"

  @justifies:SPEC030
  Scenario: H13 Ignored feature files are not scanned
    Given the file ".gitignore" contains:
      """
      /generated
      """
    And the file "generated/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ999
        Scenario: Generated
          Then the login is rejected
      """
    When I run "emperor check"
    Then the exit code is 0
