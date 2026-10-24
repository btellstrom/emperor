Feature: Review in a terminal

  Scenario: TM1 The terminal review shows the scenarios before the requirements
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ001" has not been reviewed
    And emperor is not installed
    And the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          Then the login is rejected
      """
    When I start "doorstop-review" in a terminal
    Then the screen shows "Scenario 1 of 1"
    When I press "a"
    Then the screen shows "REQ001 · 1 to review"
    When I press "a"
    Then the review ends with exit code 0
    And Doorstop reports "REQ001" as reviewed

  Scenario: TM2 A question from the terminal is saved for Claude and stops the review
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          Then the login is rejected
      """
    When I start "doorstop-review" in a terminal
    And I press "q"
    And I type "why only one wrong password?" and press Enter
    Then the review ends with exit code 0
    And the output contains "Your question about A wrong password is rejected is saved for Claude."
    And the output contains "doorstop-review"
    And the git directory contains "review/question.json" with:
      | review   | gherkin                                              |
      | item     | features/login.feature::A wrong password is rejected |
      | label    | A wrong password is rejected                         |
      | question | why only one wrong password?                         |

  Scenario: TM3 The terminal review refuses to run without a terminal
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ001" has not been reviewed
    When I run "doorstop-review"
    Then the exit code is 4
    And the error output contains "The review needs a terminal of its own: run this command in one, not through Claude."
    And Doorstop reports "REQ001" as not reviewed
