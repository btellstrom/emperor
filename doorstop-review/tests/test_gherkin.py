from doorstop_review.gherkin import justified, scenarios_in

FEATURE = """\
@justifies:REQ001
Feature: Login

  Background:
    Given a user

  @justifies:REQ002 @slow
  Scenario: Rejected
    Then the login is rejected

  Scenario Outline: Known users
    When "<name>" logs in

    Examples:
      | name  |
      | alice |

  Rule: Lockout

    Scenario: Locked
      Then the account is locked
"""


def found(tmp_path):
    path = tmp_path / "login.feature"
    path.write_text(FEATURE)
    return scenarios_in(path)


def test_finds_every_scenario_with_its_tags_and_lines(tmp_path):
    scenarios = found(tmp_path)
    assert [s["name"] for s in scenarios] == ["Rejected", "Known users", "Locked"]
    rejected = scenarios[0]
    assert rejected["lines"][rejected["start"] : rejected["end"]] == [
        "  @justifies:REQ002 @slow",
        "  Scenario: Rejected",
        "    Then the login is rejected",
    ]


def test_an_outline_keeps_its_examples(tmp_path):
    outline = found(tmp_path)[1]
    assert outline["lines"][outline["end"] - 1] == "      | alice |"


def test_the_key_names_the_file_and_the_scenario(tmp_path):
    assert found(tmp_path)[0]["key"] == f"{(tmp_path / 'login.feature').as_posix()}::Rejected"


def test_the_hash_changes_with_the_scenario_only(tmp_path):
    before = [s["hash"] for s in found(tmp_path)]
    (tmp_path / "login.feature").write_text(FEATURE.replace("account is locked", "account locks"))
    after = [s["hash"] for s in scenarios_in(tmp_path / "login.feature")]
    assert before[:2] == after[:2]
    assert before[2] != after[2]


def test_a_scenario_justifies_its_own_and_its_features_requirements(tmp_path):
    rejected, outline, _ = found(tmp_path)
    assert justified(rejected) == ["REQ001", "REQ002"]
    assert justified(outline) == ["REQ001"]
