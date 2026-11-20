from pathlib import Path
from fnmatch import fnmatchcase


def _walk(base, parts, recursive, hidden):
    if not parts:
        return [base] if base.exists() else []
    part, rest = parts[0], parts[1:]
    if part == "":
        return _walk(base, rest, recursive, hidden) if base.is_dir() else []
    if not any(c in part for c in "*?["):
        return _walk(base / part, rest, recursive, hidden)
    if not base.is_dir():
        return []
    deep = recursive and part == "**"
    hits = _walk(base, rest, recursive, hidden) if deep else []
    for child in base.iterdir():
        if not hidden and child.name.startswith(".") and not part.startswith("."):
            continue
        if deep and child.is_dir() and not child.is_symlink():
            hits.extend(_walk(child, parts, recursive, hidden))
        elif fnmatchcase(child.name, part):
            hits.extend(_walk(child, rest, recursive, hidden))
    return hits


def glob(pathname, *, root_dir=None, recursive=False, include_hidden=False):
    if not pathname:
        return []
    absolute = pathname.startswith("/")
    base = Path("/" if absolute else root_dir or ".")
    parts = pathname.lstrip("/").split("/")
    hits = _walk(base, parts, recursive, include_hidden)
    prefix = str(base).rstrip("/") + "/"
    return [str(p) if absolute else str(p).removeprefix(prefix) for p in hits if p != base or pathname in (".", "/")]


def iglob(pathname, **kwargs):
    return iter(glob(pathname, **kwargs))
