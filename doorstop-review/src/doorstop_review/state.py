"""Review state, kept in the git directory so that it never shows up in the working tree."""

import json
import pathlib
import subprocess

from .errors import ReviewError


def git_dir():
    result = subprocess.run(
        ["git", "rev-parse", "--absolute-git-dir"], capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        raise ReviewError("doorstop-review needs to run inside a git repository")
    return pathlib.Path(result.stdout.strip())


def path(name):
    return git_dir() / "review" / name


def load(name, empty):
    file = path(name)
    return json.loads(file.read_text()) if file.exists() else empty


def save(name, state):
    file = path(name)
    file.parent.mkdir(exist_ok=True)
    file.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
