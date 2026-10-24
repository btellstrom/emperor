"""Reviewers' replies such as `a`, `r too vague` or `q: why 7 days?`."""

VERDICTS = {
    "a": "accepted",
    "accept": "accepted",
    "r": "rejected",
    "reject": "rejected",
    "q": "question",
    "question": "question",
}
KEYS = "`a` accept · `r` reject (add why) · `q` ask Claude a question"
NOT_A_KEY = "Not a review key: expected `a`, `r [why]` or `q [question]`."


def interpret(message):
    """(verdict, rest): the verdict is None when the message does not start with a review key."""
    words = message.strip().split(None, 1)
    if not words:
        return None, ""
    first = words[0].lower().rstrip(":.,")
    rest = words[1].strip() if len(words) > 1 else ""
    return VERDICTS.get(first), rest
