"""emperor, when the repository uses it: a repository uses emperor when it has emperor.lock."""

import collections
import json
import os
import pathlib
import shutil
import subprocess

from .errors import ReviewError

LOCKFILE = pathlib.Path("emperor.lock")


def find():
    """The emperor binary to run, or None when the repository does not use emperor."""
    if not LOCKFILE.exists():
        return None
    found = os.environ.get("EMPEROR") or shutil.which("emperor")
    if not found:
        raise ReviewError(
            "this repository uses emperor (emperor.lock), but emperor is not on the PATH; "
            "install it, or point EMPEROR at its binary"
        )
    return found


def _run(binary, *arguments):
    result = subprocess.run([binary, *arguments], capture_output=True, text=True, check=False)
    return result.returncode, result.stdout, result.stderr


def problems(binary):
    """{requirement: [problem, ...]} as `emperor check` reports them."""
    code, output, error = _run(binary, "check", "--format", "json")
    if code not in (0, 1):
        raise ReviewError(f"emperor check failed:\n{error}")
    found = {}
    for problem in json.loads(output)["problems"]:
        found.setdefault(problem["requirement"], []).append(problem)
    return found


def accept(binary, uid):
    code, _output, error = _run(binary, "accept", uid)
    if code != 0:
        raise ReviewError(f"Doorstop now has {uid} reviewed, but emperor accept failed:\n{error}")


def summary(problems):
    """How many problems of each kind, such as `1 changed, 2 unaccepted`."""
    counts = collections.Counter(problem["kind"] for problem in problems)
    return ", ".join(f"{n} {kind}" for kind, n in sorted(counts.items()))
