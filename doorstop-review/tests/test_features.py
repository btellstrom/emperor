"""Runs the scenarios under features/ against the installed doorstop-review command."""

import json
import os
import re
import shlex
import subprocess
import sys
from pathlib import Path

import doorstop
import pexpect
import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from doorstop_review import terminal

scenarios(".")

REPOSITORY = Path(__file__).resolve().parents[2]
EMPEROR = REPOSITORY / "target" / "debug" / "emperor"
# The doorstop-review script and the Python it runs on live next to the interpreter.
VENV_BIN = Path(sys.executable).parent
TERMINAL_TIMEOUT = 10


@pytest.fixture(scope="session")
def emperor_binary():
    subprocess.run(["cargo", "build", "-q", "--bin", "emperor"], cwd=REPOSITORY, check=True)
    return EMPEROR


class Project:
    """A git repository with a Doorstop tree, in which commands are run."""

    def __init__(self, root):
        self.root = root
        self.tools = root.parent / "tools"
        self.tools.mkdir()
        self.levels = {}
        self.output = ""
        self.error = ""
        self.exit_code = None
        self.terminal = None
        subprocess.run(["git", "init", "-q", str(root)], check=True)

    def path(self, relative):
        return self.root / relative

    def write(self, relative, text):
        path = self.path(relative)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def environment(self):
        env = {key: value for key, value in os.environ.items() if key != "EMPEROR"}
        env["PATH"] = os.pathsep.join([str(VENV_BIN), str(self.tools), "/usr/bin", "/bin"])
        return env

    def document(self, prefix, parent=None, folder=None):
        lines = ["settings:", "  digits: 3", "  itemformat: yaml"]
        if parent:
            lines.append(f"  parent: {parent}")
        lines += [f"  prefix: {prefix}", "  sep: ''"]
        self.write(f"{folder or prefix.lower()}/.doorstop.yml", "\n".join(lines) + "\n")

    def requirement(self, uid, text, references=()):
        prefix = uid.rstrip("0123456789")
        level = self.levels[prefix] = self.levels.get(prefix, 0) + 1
        reference_lines = "".join(f"- path: {path}\n  type: file\n" for path in references)
        self.write(
            f"{prefix.lower()}/{uid}.yml",
            "active: true\nderived: false\nheader: ''\n"
            f"level: 1.{level}\nlinks: []\nnormative: true\nref: ''\n"
            + (f"references:\n{reference_lines}" if references else "")
            + f"reviewed: null\ntext: |\n  {text}\n",
        )

    def tree(self):
        return doorstop.build(cwd=str(self.root), root=str(self.root))

    def item(self, uid):
        return self.tree().find_item(uid)

    def run(self, command, stdin=""):
        result = subprocess.run(
            shlex.split(command),
            cwd=self.root,
            env=self.environment(),
            input=stdin,
            capture_output=True,
            text=True,
            check=False,
        )
        self.output, self.error, self.exit_code = result.stdout, result.stderr, result.returncode

    def start(self, command):
        self.terminal = pexpect.spawn(
            command,
            cwd=str(self.root),
            env=self.environment(),
            dimensions=(40, 140),
            encoding="utf-8",
            timeout=TERMINAL_TIMEOUT,
        )


@pytest.fixture
def project(tmp_path, monkeypatch):
    project = Project(tmp_path / "project")
    # Doorstop runs `git add` on every item it saves, in the current directory's repository.
    monkeypatch.chdir(project.root)
    return project


# Doorstop


@given(parsers.re(r'a Doorstop document "(?P<prefix>\w+)" with no parent'))
def document(project, prefix):
    project.document(prefix)


@given(parsers.re(r'a Doorstop document "(?P<prefix>\w+)" with parent "(?P<parent>\w+)"$'))
def child_document(project, prefix, parent):
    project.document(prefix, parent)


@given(
    parsers.re(
        r'requirement "(?P<uid>\w+)" with text "(?P<text>[^"]*)" referencing file "(?P<path>[^"]+)"'
    )
)
def requirement_referencing_file(project, uid, text, path):
    project.write(path, f"contents of {path}\n")
    project.requirement(uid, text, references=[path])


@given(parsers.re(r'requirement "(?P<uid>\w+)" with text "(?P<text>[^"]*)"$'))
def requirement(project, uid, text):
    project.requirement(uid, text)


@given(parsers.re(r'requirement "(?P<uid>\w+)" links to "(?P<parent>\w+)"'))
def link(project, uid, parent):
    project.item(uid).link(parent)


@given(
    parsers.re(
        r'requirements? "(?P<first>\w+)"(?: and "(?P<second>\w+)")? (?:has|have) not been reviewed'
    )
)
def not_reviewed(project, first, second):
    # New items have no review stamp, so this only checks the fixture.
    for uid in filter(None, (first, second)):
        assert not project.item(uid).reviewed


@given("all requirements have been reviewed")
def all_reviewed(project):
    for document_ in project.tree():
        for item in document_:
            item.clear()
            item.review()


@given(parsers.re(r'the text of "(?P<uid>\w+)" changes to "(?P<text>[^"]*)"'))
@when(parsers.re(r'the text of "(?P<uid>\w+)" changes to "(?P<text>[^"]*)"'))
def change_text(project, uid, text):
    project.item(uid).text = text


@then(parsers.re(r'Doorstop reports "(?P<uid>\w+)" as reviewed'))
def reported_reviewed(project, uid):
    assert project.item(uid).reviewed


@then(parsers.re(r'Doorstop reports "(?P<uid>\w+)" as not reviewed'))
def reported_not_reviewed(project, uid):
    assert not project.item(uid).reviewed


@then(parsers.re(r'Doorstop reports the link from "(?P<uid>\w+)" to "(?P<parent>\w+)" as cleared'))
def reported_cleared(project, uid, parent):
    tree = project.tree()
    link = next(link for link in tree.find_item(uid).links if str(link) == parent)
    assert link.stamp == tree.find_item(parent).stamp()


# Files


@given(parsers.re(r'the file "(?P<path>[^"]+)" contains:'))
def file_contains(project, path, docstring):
    project.write(path, docstring + "\n")


@given(parsers.re(r'the file "(?P<path>[^"]+)" exists'))
def file_exists(project, path):
    project.write(path, "")


@given(parsers.re(r'the file "(?P<path>[^"]+)" changes'))
def file_changes(project, path):
    project.write(path, project.path(path).read_text() + "changed\n")


@when(parsers.re(r'the line "(?P<old>[^"]+)" in "(?P<path>[^"]+)" changes to "(?P<new>[^"]+)"'))
def line_changes(project, old, path, new):
    text = project.path(path).read_text()
    assert old in text
    project.path(path).write_text(text.replace(old, new))


@given(parsers.re(r'there is no file "(?P<path>[^"]+)"'))
@then(parsers.re(r'there is no file "(?P<path>[^"]+)"'))
def no_file(project, path):
    assert not project.path(path).exists()


@given(parsers.re(r'there is no folder "(?P<path>[^"]+)"'))
def no_folder(project, path):
    assert not project.path(path).exists()


@then(parsers.re(r'the git directory contains "(?P<path>[^"]+)" with:'))
def git_file_contains(project, path, datatable):
    saved = json.loads((project.path(".git") / path).read_text())
    for key, value in datatable:
        assert saved[key] == value, key


# emperor


@given("emperor is installed")
def emperor_installed(project, emperor_binary):
    (project.tools / "emperor").symlink_to(emperor_binary)


@given("emperor is not installed")
def emperor_not_installed(project):
    assert not (project.tools / "emperor").exists()


@then(parsers.re(r'"(?P<command>[^"]+)" exits with (?P<code>\d+)'))
def command_exits_with(project, command, code):
    project.run(command)
    assert project.exit_code == int(code), project.output + project.error


# Running commands


@given(parsers.re(r'I have run "(?P<command>[^"]+)" with input "(?P<stdin>[^"]*)"'))
@when(parsers.re(r'I have run "(?P<command>[^"]+)" with input "(?P<stdin>[^"]*)"'))
@when(parsers.re(r'I run "(?P<command>[^"]+)" with input "(?P<stdin>[^"]*)"'))
def run_with_input(project, command, stdin):
    project.run(command, stdin)


@given(parsers.re(r'I have run "(?P<command>[^"]+)"$'))
@when(parsers.re(r'I have run "(?P<command>[^"]+)"$'))
@when(parsers.re(r'I run "(?P<command>[^"]+)"$'))
def run(project, command):
    project.run(command)


@then(parsers.re(r"the exit code is (?P<code>\d+)"))
def exit_code(project, code):
    assert project.exit_code == int(code), project.output + project.error


@then(parsers.re(r'the output contains "(?P<expected>.*)"$'))
def output_contains(project, expected):
    assert expected in project.output, project.output


@then(parsers.re(r'the error output contains "(?P<expected>.*)"$'))
def error_output_contains(project, expected):
    assert expected in project.error, project.error


# The terminal


@when(parsers.re(r'I start "(?P<command>[^"]+)" in a terminal'))
def start_in_terminal(project, command):
    project.start(command)


@then(parsers.re(r'the screen shows "(?P<expected>[^"]+)"'))
def screen_shows(project, expected):
    project.terminal.expect(re.escape(expected))


@when(parsers.re(r'I press "(?P<key>.)"'))
def press(project, key):
    # Like a person, wait for the screen: keys typed before it opens are discarded.
    project.terminal.expect(re.escape(terminal.HELP))
    project.terminal.send(key)


@when(parsers.re(r'I type "(?P<text>[^"]+)" and press Enter'))
def type_line(project, text):
    project.terminal.expect(re.escape("(Ctrl-C to go back)"))
    project.terminal.sendline(text)


@then(parsers.re(r"the review ends with exit code (?P<code>\d+)"))
def review_ends(project, code):
    project.terminal.expect(pexpect.EOF)
    project.terminal.close()
    project.output = project.terminal.before
    assert project.terminal.exitstatus == int(code), project.output
