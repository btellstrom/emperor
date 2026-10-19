Feature: Ask a judge whether the justifications fulfil a requirement

  # The judge is any command configured in emperor.toml. It receives the requirement
  # and its justifications as JSON on stdin and answers with a verdict as JSON on stdout.
  # An LLM is one possible judge; the tests use a scripted fake so they are deterministic.
  # Verdicts are advisory by default, because an LLM can be wrong or manipulated
  # by the code it reads.

  Background:
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password" referencing file "tests/login.rs"

  @pending @justifies:SPEC033
  Scenario: J1 The judge receives the requirement and its justifications
    Given a judge that answers "fulfilled" with reason "test covers it"
    When I run "emperor judge REQ001"
    Then the judge received:
      """
      {
        "requirement": { "id": "REQ001", "text": "Login shall reject a wrong password" },
        "justifications": [
          { "type": "file", "path": "tests/login.rs", "content": "<contents of tests/login.rs>" }
        ]
      }
      """

  @pending @justifies:SPEC034
  Scenario: J2 A fulfilled verdict is reported with its reason
    Given a judge that answers "fulfilled" with reason "test covers it"
    When I run "emperor judge REQ001"
    Then the exit code is 0
    And the output contains "REQ001: fulfilled (test covers it)"

  @pending @justifies:SPEC034
  Scenario: J3 An unfulfilled verdict is advisory by default
    Given a judge that answers "unfulfilled" with reason "no assertion on the result"
    When I run "emperor judge REQ001"
    Then the exit code is 0
    And the output contains "REQ001: unfulfilled (no assertion on the result)"

  @pending @justifies:SPEC034
  Scenario: J4 An unfulfilled verdict fails when gating is requested
    Given a judge that answers "unfulfilled" with reason "no assertion on the result"
    When I run "emperor judge REQ001 --fail-on-unfulfilled"
    Then the exit code is 1

  @pending @justifies:SPEC035
  Scenario: J5 Verdicts are cached while nothing changes
    Given a judge that answers "fulfilled" with reason "test covers it"
    And I have run "emperor judge REQ001"
    When I run "emperor judge REQ001"
    Then the judge was called 1 time
    And the output contains "REQ001: fulfilled (test covers it) [cached]"

  @pending @justifies:SPEC035
  Scenario: J6 Changing a justification invalidates the cached verdict
    Given a judge that answers "fulfilled" with reason "test covers it"
    And I have run "emperor judge REQ001"
    When "tests/login.rs" is modified
    And I run "emperor judge REQ001"
    Then the judge was called 2 times

  @pending @justifies:SPEC036
  Scenario: J7 A failing judge command is an error
    Given a judge that exits with status 3
    When I run "emperor judge REQ001"
    Then the exit code is 2
    And the error output contains "judge failed for REQ001"

  @pending @justifies:SPEC036
  Scenario: J8 Judging without a configured judge is an error
    When I run "emperor judge REQ001"
    Then the exit code is 2
    And the error output contains "no judge configured in emperor.toml"
