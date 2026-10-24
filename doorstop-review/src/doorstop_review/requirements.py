"""Review of a Doorstop tree's requirements, one at a time.

A requirement is up for review while Doorstop has no current review stamp for it, one of
its links is suspect, or (in a repository that uses emperor) emperor reports a problem
with its justifications.

Accepting a requirement clears its suspect links and stamps it as reviewed in Doorstop,
then, with emperor, runs `emperor accept` on it. Rejecting it only notes the rejection in
the git directory (review/requirements.json); it comes up again once it changes.
"""

import os
import pathlib
import textwrap

from . import emperor, gherkin, scenarios, state, tree
from .replies import KEYS, NOT_A_KEY, interpret
from .rust_source import rust_fn

STATE = "requirements.json"
FENCES = {
    ".rs": "rust",
    ".sh": "sh",
    ".yaml": "yaml",
    ".yml": "yaml",
    ".hcl": "hcl",
    ".toml": "toml",
    ".py": "python",
    ".feature": "gherkin",
}
WIDTH = 92


def load_state():
    return state.load(STATE, {"rejected": {}, "shown": None})


def save_state(current):
    state.save(STATE, current)


def stamp(item):
    return str(item.stamp(links=True))


def rejected(current, item):
    rejection = current["rejected"].get(str(item.uid))
    return rejection is not None and rejection["stamp"] == stamp(item)


class Review:
    """The tree, what emperor reports about it, and which requirements are up for review."""

    def __init__(self, current, binary):
        self.tree = tree.build()
        self.problems = emperor.problems(binary) if binary else {}
        documents = sorted(self.tree.documents, key=lambda document: document.depth)
        self.items = [item for document in documents for item in document.items if item.active]
        self.queue = [
            item for item in self.items if self.pending(item) and not rejected(current, item)
        ]
        self.scenarios = gherkin.by_requirement()
        self.verdicts = scenarios.verdicts()

    def pending(self, item):
        return not item.reviewed or not item.cleared or str(item.uid) in self.problems

    def rejected_count(self, current):
        return sum(rejected(current, item) for item in self.items)

    def scenario_verdict(self, scenario):
        return self.verdicts.get(scenario["key"], "not reviewed")

    def describe(self, item):
        """The requirement, its links, its justifications and what emperor says about them."""
        uid = str(item.uid)
        lines = [f"{uid}  {item.level}  {item.header or ''}".rstrip(), ""]
        lines += wrap(item.text, "    ")
        if item.heading:
            return lines
        lines.append("")
        for link in item.links:
            parent = tree.find(self.tree, str(link))
            lines.append(f"    Refines {tree.title(parent) if parent else str(link)}")
        named = [
            f"      {j['path'].strip()}  {j.get('item', '').strip()}".rstrip()
            for j in justified_by(item)
        ]
        named += [
            f"      {s['path']}  {s['name']}  (scenario {self.scenario_verdict(s)})"
            for s in self.scenarios.get(uid, [])
        ]
        lines.append("    Justified by" if named else "    Justified by nothing")
        lines += named
        kinds = {}
        for problem in self.problems.get(uid, []):
            kinds[problem["kind"]] = kinds.get(problem["kind"], 0) + 1
        if kinds:
            lines.append(
                "    emperor: " + ", ".join(f"{n} {kind}" for kind, n in sorted(kinds.items()))
            )
        return lines

    def context(self, item):
        """The lines above the requirement: its chapter's heading, or its document at the top."""
        chapter = chapter_of(item)
        if chapter is item:
            return []
        if chapter is None:
            return [f"{item.document.prefix}  ({os.path.relpath(item.document.path)})", ""]
        return self.describe(chapter) + [""]

    def justifications(self, item):
        """(title, fence, source or None) for the other side of each of the item's justifications."""
        found = []
        for justification in justified_by(item):
            # Doorstop writes long values as blocks, which end in a newline.
            path = justification["path"].strip()
            name = justification.get("item", "").strip()
            fence = FENCES.get(pathlib.Path(path).suffix, "")
            if justification["type"] == "rust-fn":
                found.append((f"{path} · {name}", fence, rust_fn(path, name)))
            else:
                found.append((f"{path}, the whole file", fence, read(path)))
        for scenario in self.scenarios.get(str(item.uid), []):
            block = scenario["lines"][scenario["start"] : scenario["end"]]
            found.append(
                (
                    f"{scenario['path']} · {scenario['name']} (scenario {self.scenario_verdict(scenario)})",
                    "gherkin",
                    textwrap.dedent("\n".join(block)),
                )
            )
        return found

    def summary(self, current, markdown=True):
        bold = "**" if markdown else ""
        return [f"{bold}No requirement is left to review{bold} of {len(self.items)}."] + rejections(
            current, self.items
        )

    def heading(self, current):
        return (
            f"{len(self.queue)} to review, {self.rejected_count(current)} rejected, "
            f"{len(self.items)} in all"
        )


def justified_by(item):
    return item.get("justified_by") or []


def read(path):
    try:
        return pathlib.Path(path).read_text()
    except OSError:
        return None


def rejections(current, items):
    out = []
    for item in items:
        if rejected(current, item):
            why = current["rejected"][str(item.uid)]["why"]
            out.append(f"- rejected {tree.title(item)}" + (f": {why}" if why else ""))
    return out


def chapter_of(item):
    """The heading at the top of the item's chapter, or None at the top level."""
    major = item.level.value[0]
    for other in item.document.items:
        if other.heading and other.level.value[0] == major and not any(other.level.value[1:]):
            return other
    return None


def wrap(text, indent):
    out = []
    for paragraph in text.strip().split("\n\n"):
        out += textwrap.wrap(
            " ".join(paragraph.split()),
            WIDTH - len(indent),
            initial_indent=indent,
            subsequent_indent=indent,
        ) or [indent]
        out.append("")
    return out[:-1]


def accept(item, binary):
    if item.links:
        item.clear()
    item.review()
    if binary:
        emperor.accept(binary, str(item.uid))


def decide(current, item, verdict, why, binary):
    if verdict == "accepted":
        accept(item, binary)
        current["rejected"].pop(str(item.uid), None)
    else:
        current["rejected"][str(item.uid)] = {"why": why, "stamp": stamp(item)}
    save_state(current)


class Phase:
    """The requirements, for the terminal review."""

    name = "requirements"

    def __init__(self):
        self.state = load_state()
        self.binary = emperor.find()

    def next(self):
        self.review = Review(self.state, self.binary)
        if not self.review.queue:
            return None
        current = self.review.queue[0]
        rows = [("dim", line) for line in self.review.context(current)]
        rows += [("highlight", line) for line in self.review.describe(current)]
        if not current.heading:
            for title, _fence, body in self.review.justifications(current):
                rows += [("plain", ""), ("heading", title)]
                if body is None:
                    rows.append(("note", "  Not found."))
                else:
                    rows += [("plain", "  " + line) for line in body.rstrip("\n").split("\n")]
        return {
            "title": f"{current.uid} · {self.review.heading(self.state)}",
            "label": tree.title(current),
            "key": str(current.uid),
            "rows": rows,
            "requirement": current,
        }

    def decide(self, item, verdict, why):
        decide(self.state, item["requirement"], verdict, why, self.binary)

    def summary(self):
        return self.review.summary(self.state, markdown=False)


def sources(review, item):
    """Markdown with the other side of each of the item's justifications."""
    out = []
    for title, fence, body in review.justifications(item):
        out += [f"`{title}`", ""]
        out += ["Not found.", ""] if body is None else [f"```{fence}", body.rstrip("\n"), "```", ""]
    return out


def show(current, binary, note=None):
    review = Review(current, binary)
    if note:
        print(note + "\n")
    if not review.queue:
        current["shown"] = None
        save_state(current)
        print("\n".join(review.summary(current)))
        return
    item = review.queue[0]
    current["shown"] = {"uid": str(item.uid), "stamp": stamp(item)}
    save_state(current)
    print(f"**{item.document.prefix} requirement {item.uid}**: {review.heading(current)}")
    print()
    print("```diff")
    for line in review.context(item):
        print("  " + line)
    for line in review.describe(item):
        print("+ " + line)
    print("```")
    print()
    if not item.heading:
        print("\n".join(sources(review, item)))
    print(KEYS)


def key(current, binary, message):
    """Act on the reviewer's reply; returns the exit code (0 decided, 2 not a key, 3 question)."""
    verdict, rest = interpret(message)
    if verdict is None:
        print(NOT_A_KEY)
        return 2
    shown = current.get("shown")
    item = tree.find(tree.build(), shown["uid"]) if shown else None
    if item is None or stamp(item) != shown["stamp"]:
        show(
            current,
            binary,
            "The requirement on screen has changed or gone since it was shown, "
            "so nothing was decided. Here is what is up for review now:",
        )
        return 0
    if verdict == "question":
        print(
            f"Paused at {tree.title(item)} ({os.path.relpath(item.path)}). "
            f"The reviewer asks: {rest or '(question to follow)'}"
        )
        print("Reply `a` or `r` to decide it, or say continue to see it again.")
        return 3
    decide(current, item, verdict, rest, binary)
    show(current, binary, f"{verdict.capitalize()} {tree.title(item)}.")
    return 0


def status(current, binary):
    """Print what is left; returns 1 while any requirement is still to review."""
    review = Review(current, binary)
    print(f"{len(review.queue)} of {len(review.items)} requirements still to review")
    print("\n".join(rejections(current, review.items)))
    return 1 if review.queue else 0
