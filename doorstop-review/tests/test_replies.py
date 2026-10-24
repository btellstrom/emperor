from doorstop_review.replies import interpret


def test_reads_the_verdict_and_what_follows_it():
    assert interpret("a") == ("accepted", "")
    assert interpret("accept") == ("accepted", "")
    assert interpret("r too vague") == ("rejected", "too vague")
    assert interpret("R: too vague") == ("rejected", "too vague")
    assert interpret("q, why 7 days?") == ("question", "why 7 days?")


def test_anything_else_is_not_a_review_key():
    assert interpret("") == (None, "")
    assert interpret("looks fine to me") == (None, "fine to me")
