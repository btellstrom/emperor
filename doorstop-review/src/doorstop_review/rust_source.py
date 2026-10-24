"""Rust functions in source files, for showing a test that justifies a requirement."""

import pathlib
import re
import textwrap


def skip_literal(source, i):
    """The index just past a string, char or comment starting at i, or i if none does."""
    if source.startswith("//", i):
        end = source.find("\n", i)
        return len(source) if end < 0 else end
    if source.startswith("/*", i):
        end = source.find("*/", i + 2)
        return len(source) if end < 0 else end + 2
    raw = re.match(r'b?r(#*)"', source[i : i + 260])
    if raw and (i == 0 or not (source[i - 1].isalnum() or source[i - 1] == "_")):
        end = source.find('"' + raw.group(1), i + raw.end())
        return len(source) if end < 0 else end + 1 + len(raw.group(1))
    if source[i] == '"':
        j = i + 1
        while j < len(source) and source[j] != '"':
            j += 2 if source[j] == "\\" else 1
        return j + 1
    char = re.match(r"'(?:\\.[^']*|[^\\'])'", source[i : i + 12])
    if char:
        return i + char.end()
    return i


def open_modules(source, end):
    """The names of the `mod` blocks open at index `end`."""
    i, stack = 0, []
    while i < end:
        j = skip_literal(source, i)
        if j != i:
            i = j
            continue
        if source[i] == "{":
            before = re.search(r"\bmod\s+(\w+)\s*$", source[max(0, i - 200) : i])
            stack.append(before.group(1) if before else None)
        elif source[i] == "}" and stack:
            stack.pop()
        i += 1
    return [module for module in stack if module]


def block_end(source, i):
    """The index of the brace that closes the block opening at i."""
    depth = 0
    while 0 <= i < len(source):
        j = skip_literal(source, i)
        if j != i:
            i = j
            continue
        depth += {"{": 1, "}": -1}.get(source[i], 0)
        if depth == 0:
            return i
        i += 1
    return None


def with_attributes(source, start):
    """The start of the line at `start`, moved up over attributes and comments above it."""
    start = source.rfind("\n", 0, start) + 1
    lines_before = source[:start].split("\n")[:-1]
    while lines_before and lines_before[-1].strip().startswith(("#[", "///", "//")):
        start -= len(lines_before.pop()) + 1
    return start


def rust_fn(path, item):
    """The source of the function `item` (a module path such as `tests::name`) in a file,
    with the attributes and doc comments above it, or None when it isn't there."""
    try:
        source = pathlib.Path(path).read_text()
    except OSError:
        return None
    *modules, name = item.split("::")
    for match in re.finditer(rf"\bfn\s+{re.escape(name)}\b", source):
        if open_modules(source, match.start()) != modules:
            continue
        end = block_end(source, source.find("{", match.end()))
        if end is not None:
            return textwrap.dedent(source[with_attributes(source, match.start()) : end + 1])
    return None
