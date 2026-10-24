import pytest

from doorstop_review import emperor
from doorstop_review.errors import ReviewError


@pytest.fixture
def no_emperor(monkeypatch, tmp_path):
    monkeypatch.delenv("EMPEROR", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path / "empty"))
    monkeypatch.chdir(tmp_path)


def test_a_repository_without_emperor_lock_does_not_use_emperor(no_emperor):
    assert emperor.find() is None


def test_a_repository_with_emperor_lock_uses_the_binary_emperor_points_at(
    no_emperor, monkeypatch, tmp_path
):
    (tmp_path / "emperor.lock").write_text("")
    monkeypatch.setenv("EMPEROR", "/opt/emperor")
    assert emperor.find() == "/opt/emperor"


def test_a_repository_with_emperor_lock_needs_emperor(no_emperor, tmp_path):
    (tmp_path / "emperor.lock").write_text("")
    with pytest.raises(ReviewError, match="but emperor is not on the PATH"):
        emperor.find()


def test_problems_are_grouped_by_requirement(monkeypatch):
    report = '{"problems": [{"requirement": "REQ001", "kind": "changed"}, {"requirement": "REQ001", "kind": "unaccepted"}]}'
    monkeypatch.setattr(emperor, "_run", lambda binary, *arguments: (1, report, ""))
    assert emperor.problems("emperor") == {
        "REQ001": [
            {"requirement": "REQ001", "kind": "changed"},
            {"requirement": "REQ001", "kind": "unaccepted"},
        ]
    }


def test_a_check_that_could_not_run_is_an_error(monkeypatch):
    monkeypatch.setattr(emperor, "_run", lambda binary, *arguments: (2, "", "error: bad tree"))
    with pytest.raises(ReviewError, match="emperor check failed"):
        emperor.problems("emperor")
