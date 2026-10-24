Feature: Review Gherkin scenarios

  Scenario: SC1 A scenario is shown in its feature file with the requirements it justifies
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          When "alice" logs in with "wrong"
          Then the login is rejected
      """
    When I run "doorstop-review scenarios show"
    Then the output contains "Scenario 1 of 1"
    And the output contains "+   Scenario: A wrong password is rejected"
    And the output contains "  Feature: Login"
    And the output contains "REQ001"
    And the output contains "Login shall reject a wrong password"

  Scenario: SC2 A decided scenario comes up again once it changes
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          When "alice" logs in with "wrong"
          Then the login is rejected
      """
    And I have run "doorstop-review scenarios show"
    When I run "doorstop-review scenarios key" with input "a"
    Then the exit code is 0
    And the output contains "Accepted A wrong password is rejected"
    And the output contains "All 1 scenarios are decided"
    When the line "Then the login is rejected" in "features/login.feature" changes to "Then the login is rejected with an error"
    And I run "doorstop-review scenarios show"
    Then the output contains "Scenario 1 of 1"
    And the output contains "+     Then the login is rejected with an error"

  Scenario: SC3 A rejected scenario is listed with its reason
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ001
        Scenario: A wrong password is rejected
          Then the login is rejected

        @justifies:REQ001
        Scenario: A locked account is rejected
          Then the login is rejected
      """
    And I have run "doorstop-review scenarios show"
    And I have run "doorstop-review scenarios key" with input "r which error message?"
    When I run "doorstop-review scenarios status"
    Then the exit code is 1
    And the output contains "0 accepted, 1 rejected, 1 of 2 still to review"
    And the output contains "rejected `features/login.feature::A wrong password is rejected`: which error message?"
    When I have run "doorstop-review scenarios show"
    And I have run "doorstop-review scenarios key" with input "a"
    And I run "doorstop-review scenarios status"
    Then the exit code is 0
    And the output contains "1 accepted, 1 rejected, 0 of 2 still to review"

  Scenario: SC4 A scenario naming a requirement that does not exist says so
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And the file "features/login.feature" contains:
      """
      Feature: Login

        @justifies:REQ999
        Scenario: A wrong password is rejected
          Then the login is rejected
      """
    When I run "doorstop-review scenarios show"
    Then the exit code is 0
    And the output contains "**REQ999**: no such requirement"

  Scenario: SC5 A repository without feature files has no scenarios to review
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And there is no folder "features"
    When I run "doorstop-review scenarios show"
    Then the exit code is 0
    And the output contains "There are no scenarios to review: no .feature files under features/."
    When I run "doorstop-review scenarios status"
    Then the exit code is 0
