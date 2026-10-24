Feature: Review requirements with emperor

  Scenario: RV8 A reviewed requirement whose justification changed is shown, and accepting runs emperor accept
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password" referencing file "docs/login.md"
    And all requirements have been reviewed
    And emperor is installed
    And I have run "emperor accept REQ001"
    And the file "docs/login.md" changes
    When I run "doorstop-review requirements show"
    Then the output contains "REQ001"
    And the output contains "emperor: 1 changed"
    When I run "doorstop-review requirements key" with input "a"
    Then the output contains "No requirement is left to review"
    And "emperor check" exits with 0

  Scenario: RV9 A repository without emperor.lock is reviewed with Doorstop only
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password" referencing file "docs/login.md"
    And requirement "REQ001" has not been reviewed
    And emperor is installed
    And there is no file "emperor.lock"
    And I have run "doorstop-review requirements show"
    When I run "doorstop-review requirements key" with input "a"
    Then the output contains "No requirement is left to review"
    And Doorstop reports "REQ001" as reviewed
    And there is no file "emperor.lock"

  Scenario: RV10 A repository with emperor.lock needs emperor to be found
    Given a Doorstop document "REQ" with no parent
    And requirement "REQ001" with text "Login shall reject a wrong password" referencing file "docs/login.md"
    And requirement "REQ001" has not been reviewed
    And the file "emperor.lock" exists
    And emperor is not installed
    When I run "doorstop-review requirements show"
    Then the exit code is 4
    And the error output contains "this repository uses emperor (emperor.lock), but emperor is not on the PATH; install it, or point EMPEROR at its binary"
