"""Review of the Gherkin scenarios under features/, one at a time.

Decisions are kept in the git directory (review/gherkin.json), keyed by `path::scenario`
with a hash of the scenario's lines: a scenario that changes after it was decided comes
up for review again.
"""

from . import gherkin, state, tree
from .replies import KEYS, NOT_A_KEY, interpret

STATE = "gherkin.json"
NO_SCENARIOS = "There are no scenarios to review: no .feature files under features/."


def load_state():
    return state.load(STATE, {"decisions": {}, "shown": None})


def save_state(current):
    state.save(STATE, current)


def decided(current, scenario):
    decision = current["decisions"].get(scenario["key"])
    return decision is not None and decision["hash"] == scenario["hash"]


def counts(current, scenarios):
    verdicts = [current["decisions"][s["key"]]["verdict"] for s in scenarios if decided(current, s)]
    return verdicts.count("accepted"), verdicts.count("rejected")


def rejections(current, scenarios, code=""):
    """A line for each rejected scenario, with the reason given."""
    out = []
    for scenario in scenarios:
        decision = current["decisions"].get(scenario["key"])
        if decided(current, scenario) and decision["verdict"] == "rejected":
            why = f": {decision['why']}" if decision["why"] else ""
            out.append(f"- rejected {code}{scenario['key']}{code}{why}")
    return out


def summary(current, scenarios, markdown=True):
    accepted, rejected = counts(current, scenarios)
    bold, code = ("**", "`") if markdown else ("", "")
    decided_line = (
        f"{bold}All {len(scenarios)} scenarios are decided{bold}: "
        f"{accepted} accepted, {rejected} rejected."
    )
    return [decided_line, *rejections(current, scenarios, code)]


def decide(current, scenario, verdict, why):
    current["decisions"][scenario["key"]] = {
        "verdict": verdict,
        "why": why,
        "hash": scenario["hash"],
    }
    save_state(current)


def verdicts():
    """{scenario key: verdict} as decided so far, whatever the scenario looks like now."""
    return {key: d["verdict"] for key, d in load_state()["decisions"].items()}


def requirement_texts(uids):
    """(title, text) for each requirement a scenario justifies; text is None for one that's gone."""
    doorstop_tree = tree.build()
    found = []
    for uid in uids:
        item = tree.find(doorstop_tree, uid)
        if item is None:
            found.append((uid, None))
        else:
            found.append((tree.title(item), " ".join(item.text.split())))
    return found


def position(scenarios, pending):
    return f"{len(scenarios) - len(pending) + 1} of {len(scenarios)}"


class Phase:
    """The scenarios, for the terminal review."""

    name = "gherkin"

    def __init__(self):
        self.state = load_state()

    def next(self):
        scenarios = gherkin.all_scenarios()
        pending = [s for s in scenarios if not decided(self.state, s)]
        if not pending:
            return None
        current = pending[0]
        accepted, rejected = counts(self.state, scenarios)
        rows = [
            ("highlight" if current["start"] <= i < current["end"] else "dim", line)
            for i, line in enumerate(current["lines"])
        ]
        return {
            "title": f"Scenario {position(scenarios, pending)} · {current['path']} · "
            f"{accepted} accepted, {rejected} rejected",
            "label": current["name"],
            "key": current["key"],
            "rows": rows,
            "pinned": pinned(requirement_texts(gherkin.justified(current))),
            "scenario": current,
        }

    def decide(self, item, verdict, why):
        decide(self.state, item["scenario"], verdict, why)

    def summary(self):
        return summary(self.state, gherkin.all_scenarios(), markdown=False)


def pinned(texts):
    """The rows at the bottom of the screen: the requirements the scenario justifies."""
    if not texts:
        return [("note", "The scenario justifies no requirement.")]
    rows = [("heading", "Justifies")]
    for title, text in texts:
        if text is None:
            rows.append(("note", f"  {title}: no such requirement"))
        else:
            rows += [("heading", f"  {title}"), ("plain", f"    {text}")]
    return rows


def requirements(texts):
    """Markdown for the requirements a scenario justifies, as the other side of each link."""
    if not texts:
        return ["The scenario justifies no requirement."]
    out = ["**Justifies**", ""]
    for title, text in texts:
        out.append(
            f"- **{title}**: no such requirement" if text is None else f"- **{title}**: {text}"
        )
    return out


def show(current, note=None):
    scenarios = gherkin.all_scenarios()
    if note:
        print(note + "\n")
    if not scenarios:
        print(NO_SCENARIOS)
        return
    pending = [s for s in scenarios if not decided(current, s)]
    if not pending:
        current["shown"] = None
        save_state(current)
        print("\n".join(summary(current, scenarios)))
        return
    scenario = pending[0]
    current["shown"] = {"key": scenario["key"], "hash": scenario["hash"]}
    save_state(current)
    accepted, rejected = counts(current, scenarios)
    print(
        f"**Scenario {position(scenarios, pending)}**: `{scenario['path']}` · "
        f"{accepted} accepted, {rejected} rejected"
    )
    print()
    print("```diff")
    for i, line in enumerate(scenario["lines"]):
        print(("+ " if scenario["start"] <= i < scenario["end"] else "  ") + line)
    print("```")
    print()
    print("\n".join(requirements(requirement_texts(gherkin.justified(scenario)))))
    print()
    print(KEYS)


def key(current, message):
    """Act on the reviewer's reply; returns the exit code (0 decided, 2 not a key, 3 question)."""
    verdict, rest = interpret(message)
    if verdict is None:
        print(NOT_A_KEY)
        return 2
    shown = current.get("shown")
    scenario = next(
        (s for s in gherkin.all_scenarios() if shown and s["key"] == shown["key"]), None
    )
    if scenario is None or scenario["hash"] != shown["hash"]:
        show(
            current,
            "The scenario on screen has changed or gone since it was shown, "
            "so nothing was decided. Here it is as it stands now:",
        )
        return 0
    if verdict == "question":
        print(f"Paused at `{scenario['key']}`. The reviewer asks: {rest or '(question to follow)'}")
        print(
            f"Lines {scenario['start'] + 1}–{scenario['end']} of {scenario['path']}. "
            "Reply `a` or `r` to decide it, or say continue to see it again."
        )
        return 3
    decide(current, scenario, verdict, rest)
    show(current, f"{verdict.capitalize()} {scenario['name']}.")
    return 0


def status(current):
    """Print what has been decided; returns 1 while any scenario is undecided."""
    scenarios = gherkin.all_scenarios()
    accepted, rejected = counts(current, scenarios)
    left = len(scenarios) - accepted - rejected
    print(f"{accepted} accepted, {rejected} rejected, {left} of {len(scenarios)} still to review")
    print("\n".join(rejections(current, scenarios, "`")))
    return 1 if left else 0
