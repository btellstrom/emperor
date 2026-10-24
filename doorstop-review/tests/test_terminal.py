from doorstop_review import terminal


class FakeScreen:
    """Answers with the given keys and prompt replies, and remembers the status lines."""

    def __init__(self, keys, replies=()):
        self.keys, self.replies, self.statuses = list(keys), list(replies), []

    def size(self):
        return 80, 24

    def draw(self, rows, top, status, pinned=()):
        self.statuses.append(status)

    def key(self):
        return self.keys.pop(0)

    def prompt(self, text):
        return self.replies.pop(0)

    def message(self, text):
        pass


class FakePhase:
    name = "fake"

    def __init__(self, count):
        self.items = [
            {"title": f"Item {n}", "label": f"item {n}", "key": str(n), "rows": [("plain", "x")]}
            for n in range(count)
        ]
        self.decisions = []

    def next(self):
        decided = {key for key, _, _ in self.decisions}
        return next((item for item in self.items if item["key"] not in decided), None)

    def decide(self, item, verdict, why):
        self.decisions.append((item["key"], verdict, why))


def test_each_item_is_decided_in_turn_until_none_is_left():
    phase = FakePhase(2)
    outcome = terminal.review(FakeScreen(["a", "r"], replies=["too vague"]), phase)
    assert outcome == ("done", None)
    assert phase.decisions == [("0", "accepted", ""), ("1", "rejected", "too vague")]


def test_scrolling_decides_nothing_and_x_stops():
    phase = FakePhase(1)
    outcome = terminal.review(FakeScreen(["j", "\x1b[6~", "G", "x"]), phase)
    assert outcome == ("stopped", phase.items[0])
    assert phase.decisions == []


def test_a_cancelled_rejection_goes_back_to_the_item():
    phase = FakePhase(1)
    terminal.review(FakeScreen(["r", "a"], replies=[None]), phase)
    assert phase.decisions == [("0", "accepted", "")]


def test_a_question_stops_the_review(monkeypatch):
    asked = []
    monkeypatch.setattr(
        terminal, "ask_claude", lambda phase, item, question: asked.append(question)
    )
    phase = FakePhase(1)
    assert terminal.review(FakeScreen(["q"], replies=["why?"]), phase) == (
        "question",
        phase.items[0],
    )
    assert asked == ["why?"]


def test_long_lines_wrap_under_their_own_indent():
    assert terminal.wrapped([("plain", "    one two three four")], 14) == [
        ("plain", "    one two"),
        ("plain", "      three"),
        ("plain", "      four"),
    ]
