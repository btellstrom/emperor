"""The Doorstop tree of the repository the review runs in."""

import doorstop

from .errors import ReviewError


def build():
    try:
        return doorstop.build()
    except doorstop.DoorstopError as error:
        raise ReviewError(f"Doorstop cannot load the tree: {error}") from error


def find(tree, uid):
    """The item with this UID, or None when there is none."""
    try:
        return tree.find_item(uid)
    except doorstop.DoorstopError:
        return None


def title(item):
    """An item's UID with its header, if it has one."""
    return " ".join(part for part in (str(item.uid), item.header) if part)
