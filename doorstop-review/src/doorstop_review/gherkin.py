"""Scenarios in Gherkin feature files, found by their lines."""

import hashlib
import pathlib
import re

SCENARIO = re.compile(r"\s*(?:Scenario|Scenario Outline|Scenario Template|Example):\s*(.*)")
# Lines that end the scenario before them. `Examples:` is not one: it belongs to its outline.
BOUNDARY = re.compile(
    r"\s*(?:Feature|Rule|Background|Scenario|Scenario Outline|Scenario Template|Example):"
)
JUSTIFIES = re.compile(r"@justifies:(\S+)")
DOC_STRING_DELIMITERS = ('"""', "```")
FEATURES = pathlib.Path("features")


def with_tags(lines, i):
    """The first line of the block starting at line i, including the tags above it."""
    while i > 0 and lines[i - 1].strip().startswith("@"):
        i -= 1
    return i


def outside_doc_strings(lines):
    """(index, line) for every line that is not part of a doc string."""
    delimiter = None
    for i, line in enumerate(lines):
        stripped = line.strip()
        if delimiter is None and stripped.startswith(DOC_STRING_DELIMITERS):
            delimiter = stripped[:3]
        elif delimiter is not None and stripped.startswith(delimiter):
            delimiter = None
        elif delimiter is None:
            yield i, line


def scenarios_in(path):
    """Every scenario in a feature file: its key, name, lines and a hash of them."""
    lines = path.read_text().splitlines()
    gherkin_lines = list(outside_doc_strings(lines))
    starts = [with_tags(lines, i) for i, line in gherkin_lines if BOUNDARY.match(line)]
    found = []
    for i, line in gherkin_lines:
        match = SCENARIO.match(line)
        if not match:
            continue
        name = match.group(1).strip()
        start = with_tags(lines, i)
        end = min((s for s in starts if s > i), default=len(lines))
        while end > start and not lines[end - 1].strip():
            end -= 1
        found.append(
            {
                "key": f"{path.as_posix()}::{name}",
                "path": path,
                "name": name,
                "start": start,
                "end": end,
                "lines": lines,
                "hash": hashlib.sha256("\n".join(lines[start:end]).encode()).hexdigest(),
            }
        )
    return found


def all_scenarios():
    """The scenarios of every feature file under features/, in path order."""
    return [s for path in sorted(FEATURES.rglob("*.feature")) for s in scenarios_in(path)]


def justified(scenario):
    """The requirements a scenario's tags, and its feature's, say it justifies."""
    lines = scenario["lines"]
    feature = next((i for i, line in enumerate(lines) if line.strip().startswith("Feature:")), None)
    tags = [] if feature is None else lines[with_tags(lines, feature) : feature]
    for line in lines[scenario["start"] : scenario["end"]]:
        if not line.strip().startswith("@"):
            break
        tags.append(line)
    return list(dict.fromkeys(uid for line in tags for uid in JUSTIFIES.findall(line)))


def by_requirement():
    """{requirement: [scenario, ...]} from the @justifies tags under features/."""
    found = {}
    for scenario in all_scenarios():
        for uid in justified(scenario):
            found.setdefault(uid, []).append(scenario)
    return found
