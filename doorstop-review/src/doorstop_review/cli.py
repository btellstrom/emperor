"""The doorstop-review command.

usage: doorstop-review                         review the scenarios, then the requirements,
                                               in the terminal
       doorstop-review scenarios|requirements review
                                               review one half in the terminal
       doorstop-review scenarios|requirements show
                                               the item up for review, as Markdown
       doorstop-review scenarios|requirements key < MESSAGE
                                               act on the reviewer's reply: a, r [why] or q [question]
       doorstop-review scenarios|requirements status
                                               what has been decided so far

Run it from the root of the repository. Exit codes: `key` 0 decided, 2 not a review key,
3 a question for Claude; `status` 0 nothing left, 1 items still to review; 4 an error.
"""

import sys

import doorstop

from . import gherkin, requirements, scenarios, terminal
from .errors import ReviewError

ERROR = 4
AGAIN = "doorstop-review"
HALVES = {"scenarios": scenarios, "requirements": requirements}
COMMANDS = ("review", "show", "key", "status")


def review_everything():
    """Scenarios first: accepting a requirement accepts the scenarios that justify it."""
    terminal.require_terminal()
    # Created first so that a missing emperor stops the review before anything is decided.
    requirement_phase = requirements.Phase()
    if gherkin.all_scenarios():
        scenario_phase = scenarios.Phase()
        outcome, item = terminal.run(scenario_phase)
        if outcome != "done":
            terminal.finish(outcome, item, scenario_phase, AGAIN)
            return 0
        print("\n".join(scenario_phase.summary()))
        if not go_on_after_rejections(scenario_phase):
            return 0
    outcome, item = terminal.run(requirement_phase)
    terminal.finish(outcome, item, requirement_phase, AGAIN)
    if outcome == "done":
        changed = "the Doorstop items" + (" and emperor.lock" if requirement_phase.binary else "")
        print(f"\nAccepting changed {changed}; commit them when you're ready.")
    return 0


def go_on_after_rejections(phase):
    _accepted, rejected = scenarios.counts(phase.state, gherkin.all_scenarios())
    if not rejected:
        return True
    print(
        f"\n{rejected} rejected. Claude can rewrite them from your reasons: send /review in "
        "Claude Code.\nThe requirements they justify will show them as rejected."
    )
    try:
        answer = input("Go on to the requirements anyway? [y/N] ")
    except EOFError, KeyboardInterrupt:
        answer = ""
    return answer.strip().lower() in ("y", "yes")


def review_half(half, command):
    module = HALVES[half]
    if command == "review":
        phase = module.Phase()
        outcome, item = terminal.run(phase)
        terminal.finish(outcome, item, phase, f"{AGAIN} {half} review")
        return 0
    current = module.load_state()
    if command == "show":
        module.show(current)
        return 0
    if command == "key":
        return module.key(current, sys.stdin.read())
    return module.status(current)


def main():
    arguments = sys.argv[1:]
    try:
        if not arguments:
            sys.exit(review_everything())
        half, command = arguments[0], arguments[1] if len(arguments) > 1 else "show"
        if len(arguments) > 2 or half not in HALVES or command not in COMMANDS:
            print(__doc__, file=sys.stderr)
            sys.exit(ERROR)
        sys.exit(review_half(half, command))
    except (ReviewError, doorstop.DoorstopError) as error:
        print(error, file=sys.stderr)
        sys.exit(ERROR)
