from pathlib import Path
from _tuse_fs import which


def copyfile(src, dst):
    source, target = Path(src), Path(dst)
    if target.exists() and source.resolve() == target.resolve():
        raise ValueError("source and destination are the same file")
    target.write_bytes(source.read_bytes())
    return dst
