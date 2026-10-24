"""A full-screen terminal review: one item at a time, decided with single keys.

A phase supplies the items. `phase.next()` returns the next item to review, or None when
none is left; `phase.decide(item, verdict, why)` records `accepted` or `rejected`; and
`phase.summary()` returns the lines to print once the phase is over. An item is a dict
with `title` (the status line), `label` (what it is called in a question to Claude),
`key` (what identifies it) and `rows`: (style, text) pairs, where style is one of
STYLES and the first `highlight` row is scrolled into view. Rows under `pinned` stay at
the bottom of the screen, above the status line, however far the rest is scrolled.

On `q` the question and the item go to the git directory (review/question.json) for
Claude to read, and the review stops there.
"""

import datetime
import os
import select
import shutil
import sys
import termios
import textwrap
import tty

from . import state
from .errors import ReviewError

RESET = "\x1b[0m"
STYLES = {
    "plain": "",
    "dim": "\x1b[2m",
    "highlight": "\x1b[1m",
    "heading": "\x1b[1;36m",
    "note": "\x1b[33m",
}
GUTTERS = {"highlight": "\x1b[1;32m▌\x1b[0m "}
HELP = "a accept · r reject · q ask Claude · ↑↓ PgUp PgDn scroll · x stop"
QUESTION = "question.json"

SCROLL = {"\x1b[A": -1, "k": -1, "\x1b[B": 1, "j": 1}
PAGE = {"\x1b[5~": -1, "b": -1, "\x1b[6~": 1, " ": 1}
STOP = ("x", "\x03", "\x04")


def ask_claude(phase, item, question):
    state.save(
        QUESTION,
        {
            "review": phase.name,
            "item": item["key"],
            "label": item["label"],
            "question": question,
            "asked": datetime.datetime.now().isoformat(timespec="seconds"),
        },
    )


def wrapped(rows, width):
    """The rows cut to the screen's width, a long line continuing under its own indent."""
    out = []
    for style, text in rows:
        text = text.expandtabs(4).rstrip()
        if len(text) <= width:
            out.append((style, text))
            continue
        indent = " " * (len(text) - len(text.lstrip()) + 2)
        out += [
            (style, line)
            for line in textwrap.wrap(text, width, subsequent_indent=indent, break_on_hyphens=False)
        ]
    return out


def pinned_rows(item, columns, lines):
    """The item's pinned rows, wrapped, and cut to half the screen at most."""
    rows = wrapped(item.get("pinned", []), columns - 2)
    limit = max(0, (lines - 3) // 2)
    if len(rows) <= limit:
        return rows
    return rows[: limit - 1] + [("note", "  … (the rest is cut off)")]


def body_height(lines, pinned):
    """The scrolling rows' share of the screen, after the pinned rows and the two status lines."""
    return max(1, lines - 2 - (len(pinned) + 1 if pinned else 0))


class Screen:
    """The terminal, switched to its alternate screen and to reading single keys."""

    def __init__(self):
        self.fd = sys.stdin.fileno()
        self.saved = termios.tcgetattr(self.fd)

    def __enter__(self):
        tty.setcbreak(self.fd)
        sys.stdout.write("\x1b[?1049h\x1b[?25l")
        return self

    def __exit__(self, *_):
        termios.tcsetattr(self.fd, termios.TCSADRAIN, self.saved)
        sys.stdout.write("\x1b[?25h\x1b[?1049l")
        sys.stdout.flush()

    def size(self):
        return tuple(shutil.get_terminal_size())

    def draw(self, rows, top, status, pinned=()):
        columns, lines = self.size()
        height = body_height(lines, pinned)
        shown = rows[top : top + height]
        out = ["\x1b[H"]
        out += [self.row(style, text) for style, text in shown]
        out += ["\x1b[2K\n"] * (height - len(shown))
        if pinned:
            out.append(f"\x1b[2K\x1b[2m{'─' * columns}{RESET}\n")
            out += [self.row(style, text) for style, text in pinned]
        out.append(f"\x1b[2K\x1b[7m {status[: columns - 2].ljust(columns - 2)} {RESET}\n")
        out.append(f"\x1b[2K\x1b[2m {HELP[: columns - 2]}{RESET}")
        sys.stdout.write("".join(out))
        sys.stdout.flush()

    @staticmethod
    def row(style, text):
        return f"\x1b[2K{GUTTERS.get(style, '  ')}{STYLES[style]}{text}{RESET}\n"

    def key(self):
        """One keypress, with arrow and page keys read whole."""
        data = os.read(self.fd, 1)
        if data == b"\x1b":
            while select.select([self.fd], [], [], 0.03)[0]:
                data += os.read(self.fd, 1)
        return data.decode(errors="replace")

    def prompt(self, text):
        """A line typed on the status line, or None when it is cancelled."""
        _columns, lines = self.size()
        sys.stdout.write(f"\x1b[{lines};1H\x1b[2K\x1b[?25h")
        sys.stdout.flush()
        termios.tcsetattr(self.fd, termios.TCSADRAIN, self.saved)
        try:
            return input(text).strip()
        except EOFError, KeyboardInterrupt:
            return None
        finally:
            tty.setcbreak(self.fd)
            sys.stdout.write("\x1b[?25l")

    def message(self, text):
        columns, lines = self.size()
        sys.stdout.write(
            f"\x1b[{lines};1H\x1b[2K\x1b[7m {text[: columns - 2].ljust(columns - 2)} {RESET}"
        )
        sys.stdout.flush()


def require_terminal():
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        raise ReviewError(
            "The review needs a terminal of its own: run this command in one, not through Claude."
        )


def run(phase):
    """Review the phase's items until none is left ("done"), the reviewer stops ("stopped")
    or asks Claude a question ("question")."""
    require_terminal()
    state.path(QUESTION).unlink(missing_ok=True)
    with Screen() as screen:
        try:
            return review(screen, phase)
        except KeyboardInterrupt:
            return "stopped", None


class View:
    """An item laid out for the screen, scrolled to `top`."""

    def __init__(self, item):
        self.item, self.size, self.top = item, None, None

    def fit(self, size):
        """Wrap again whenever the terminal is resized, keeping the place."""
        if size == self.size:
            return
        self.size = size
        self.rows = wrapped(self.item["rows"], size[0] - 2)
        self.pinned = pinned_rows(self.item, *size)
        self.height = body_height(size[1], self.pinned)
        self.last_top = max(0, len(self.rows) - self.height)
        if self.top is None:
            first = next((i for i, (style, _) in enumerate(self.rows) if style == "highlight"), 0)
            self.top = max(0, first - 3)
        self.top = min(self.top, self.last_top)

    def scroll(self, key):
        """Scrolls for a scrolling key, and says whether it was one."""
        if key in SCROLL:
            self.top += SCROLL[key]
        elif key in PAGE:
            self.top += PAGE[key] * max(1, self.height - 1)
        elif key in ("g", "\x1b[H"):
            self.top = 0
        elif key in ("G", "\x1b[F"):
            self.top = self.last_top
        else:
            return False
        self.top = min(max(0, self.top), self.last_top)
        return True


def review(screen, phase):
    item, note = phase.next(), ""
    while item is not None:
        view = View(item)
        while True:
            view.fit(screen.size())
            screen.draw(view.rows, view.top, f"{item['title']}   {note}".rstrip(), view.pinned)
            key = screen.key()
            if view.scroll(key):
                continue
            if key == "a":
                screen.message(f"Accepting {item['label']}…")
                phase.decide(item, "accepted", "")
                note = f"(accepted {item['label']})"
                break
            if key == "r":
                why = screen.prompt("Why reject it? (Enter to leave it blank, Ctrl-C to go back) ")
                if why is None:
                    continue
                phase.decide(item, "rejected", why)
                note = f"(rejected {item['label']})"
                break
            if key == "q":
                question = screen.prompt("Your question for Claude (Ctrl-C to go back): ")
                if not question:
                    continue
                ask_claude(phase, item, question)
                return "question", item
            if key in STOP:
                return "stopped", item
        item = phase.next()
    return "done", None


def finish(outcome, item, phase, again):
    """Print what happened once the screen is gone."""
    label = item["label"] if item else "the item on screen"
    if outcome == "question":
        print(f"Your question about {label} is saved for Claude.")
        print("Go back to Claude Code and send /review: Claude reads the question and answers it.")
        print(f"To carry on reviewing afterwards, run:\n  {again}")
    elif outcome == "stopped":
        print(f"Stopped at {label}. Nothing about it was decided. To carry on, run:\n  {again}")
    else:
        print("\n".join(phase.summary()))
