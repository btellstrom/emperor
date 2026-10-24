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


def test_scenarios_inside_doc_strings_are_not_scenarios(tmp_path):
    path = tmp_path / "fixtures.feature"
    path.write_text(
        "Feature: Fixtures\n\n  Scenario: Writes a feature file\n    Given the file contains:\n"
        '      """\n      Feature: Inner\n\n        Scenario: Inner one\n          Then x\n      """\n'
        "    Then it is read\n\n  Scenario: Second\n    Then y\n"
    )
    scenarios = scenarios_in(path)
    assert [s["name"] for s in scenarios] == ["Writes a feature file", "Second"]
    assert scenarios[0]["lines"][scenarios[0]["end"] - 1] == "    Then it is read"


def test_a_file_without_a_feature_line_justifies_only_what_its_scenarios_name(tmp_path):
    path = tmp_path / "loose.feature"
    path.write_text("@justifies:REQ001\nScenario: Loose\n  Then x\n")
    assert justified(scenarios_in(path)[0]) == ["REQ001"]
