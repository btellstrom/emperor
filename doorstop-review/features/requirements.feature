Feature: Review requirements with only Doorstop installed

  Scenario: RV1 An unreviewed requirement is shown for review
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ001" has not been reviewed
    And emperor is not installed
    When I run "doorstop-review requirements show"
    Then the output contains "REQ001"
    And the output contains "Login shall reject a wrong password"
    And the output contains "`a` accept · `r` reject (add why) · `q` ask Claude a question"

  Scenario: RV2 Accepting a requirement stamps it as reviewed in Doorstop
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ001" has not been reviewed
    And emperor is not installed
    And I have run "doorstop-review requirements show"
    When I run "doorstop-review requirements key" with input "a"
    Then the exit code is 0
    And the output contains "Accepted REQ001"
    And the output contains "No requirement is left to review"
    And Doorstop reports "REQ001" as reviewed

  Scenario: RV3 A requirement with a suspect link is shown, and accepting clears the link
    Given a Doorstop document "SYS" with no parent
    And a Doorstop document "REQ" with parent "SYS"
    And requirement "SYS001" with text "Users shall log in securely"
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ001" links to "SYS001"
    And all requirements have been reviewed
    And the text of "SYS001" changes to "Users shall log in with two factors"
    And emperor is not installed
    And I have run "doorstop-review requirements show"
    And I have run "doorstop-review requirements key" with input "a"
    When I run "doorstop-review requirements show"
    Then the output contains "REQ001"
    And the output contains "Refines SYS001"
    When I run "doorstop-review requirements key" with input "a"
    Then Doorstop reports the link from "REQ001" to "SYS001" as cleared

  Scenario: RV4 A rejected requirement is not shown again until it changes
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ001" has not been reviewed
    And emperor is not installed
    And I have run "doorstop-review requirements show"
    When I run "doorstop-review requirements key" with input "r too vague: which error message?"
    Then the exit code is 0
    And the output contains "Rejected REQ001"
    And the output contains "rejected REQ001"
    And the output contains "too vague: which error message?"
    And Doorstop reports "REQ001" as not reviewed
    When the text of "REQ001" changes to "Login shall reject a wrong password with 'invalid credentials'"
    And I run "doorstop-review requirements show"
    Then the output contains "REQ001"
    And the output contains "invalid credentials"

  Scenario: RV5 A question pauses the review without deciding anything
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ001" has not been reviewed
    And emperor is not installed
    And I have run "doorstop-review requirements show"
    When I run "doorstop-review requirements key" with input "q why not lock the account?"
    Then the exit code is 3
    And the output contains "Paused at REQ001"
    And the output contains "why not lock the account?"
    And Doorstop reports "REQ001" as not reviewed
    When I run "doorstop-review requirements key" with input "looks fine to me"
    Then the exit code is 2
    And the output contains "Not a review key"
    And Doorstop reports "REQ001" as not reviewed

  Scenario: RV6 Status lists what is left and the reasons for rejections
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ002" with text "Login shall be fast"
    And requirements "REQ001" and "REQ002" have not been reviewed
    And emperor is not installed
    And I have run "doorstop-review requirements show"
    And I have run "doorstop-review requirements key" with input "r too vague"
    When I run "doorstop-review requirements status"
    Then the exit code is 1
    And the output contains "1 of 2 requirements still to review"
    And the output contains "rejected REQ001"
    And the output contains "too vague"
    When I have run "doorstop-review requirements show"
    And I have run "doorstop-review requirements key" with input "a"
    And I run "doorstop-review requirements status"
    Then the exit code is 0

  Scenario: RV7 A key for a requirement that changed since it was shown decides nothing
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password"
    And requirement "REQ001" has not been reviewed
    And emperor is not installed
    And I have run "doorstop-review requirements show"
    And the text of "REQ001" changes to "Login shall accept any password"
    When I run "doorstop-review requirements key" with input "a"
    Then the exit code is 0
    And the output contains "has changed or gone since it was shown, so nothing was decided"
    And the output contains "Login shall accept any password"
    And Doorstop reports "REQ001" as not reviewed
