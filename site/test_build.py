import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PY = sys.executable


def run_build(repo_dir: Path, out: Path | str | None = None, base: str = "/tuse/"):
    out_arg = str(out if out is not None else repo_dir / "out")
    return subprocess.run(
        [PY, str(repo_dir / "site/build.py"), "--base", base, "--out", out_arg, "--check"],
        cwd=str(repo_dir),
        capture_output=True,
        text=True,
    )


def copy_repo(dst: Path) -> None:
    shutil.copytree(ROOT / "docs", dst / "docs")
    shutil.copytree(ROOT / "site", dst / "site")


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="tuse-site-test-") as td_raw:
        td = Path(td_raw)

        clean = td / "clean"
        copy_repo(clean)
        r = run_build(clean, clean / "dist", "/tuse/")
        assert r.returncode == 0, r.stderr
        r = run_build(clean, clean / "dist", "/tuse/")
        assert r.returncode == 0, r.stderr
        r = run_build(clean, clean / "dist-root", "/")
        assert r.returncode == 0, r.stderr
        assert (clean / "dist/index.html").is_file()
        assert (clean / "dist/docs/index.html").is_file()
        assert (clean / "dist/assets/demo.svg").is_file()
        assert (clean / "dist/assets/demo-mobile.svg").is_file()
        assert (clean / "dist/assets/style.css").is_file()

        unrelated = td / "keep_me"
        unrelated.mkdir()
        keep_file = unrelated / "important.txt"
        keep_file.write_text("do not delete\n")
        r = run_build(clean, unrelated)
        assert r.returncode != 0 and keep_file.read_text() == "do not delete\n", (r.returncode, r.stderr)

        r = run_build(clean, ".")
        assert r.returncode != 0 and (clean / "docs/index.md").is_file(), (r.returncode, r.stderr)

        r = run_build(clean, clean.parent)
        assert r.returncode != 0 and (clean / "docs/index.md").is_file(), (r.returncode, r.stderr)

        sym = td / "sym_out"
        os.symlink(unrelated, sym)
        r = run_build(clean, sym)
        assert r.returncode != 0 and keep_file.read_text() == "do not delete\n", (r.returncode, r.stderr)

        c_link = td / "broken_link"
        copy_repo(c_link)
        p = c_link / "docs/env.md"
        p.write_text(p.read_text() + "\n[missing](does-not-exist.md)\n")
        r = run_build(c_link)
        assert r.returncode != 0 and "does-not-exist.html" in r.stderr, r.stderr

        c_anc = td / "bad_anchor"
        copy_repo(c_anc)
        p = c_anc / "docs/env.md"
        p.write_text(p.read_text() + "\n[bad](config.md#no-such-heading)\n")
        r = run_build(c_anc)
        assert r.returncode != 0 and "no-such-heading" in r.stderr, r.stderr

        c_asset = td / "bad_asset"
        copy_repo(c_asset)
        (c_asset / "site/assets/demo-mobile.svg").unlink()
        r = run_build(c_asset)
        assert r.returncode != 0 and "demo-mobile.svg" in r.stderr, r.stderr

        c_orphan = td / "orphan"
        copy_repo(c_orphan)
        (c_orphan / "docs/orphan.md").write_text("# Orphan\n\nUnlinked.\n")
        r = run_build(c_orphan)
        assert r.returncode != 0 and "orphan" in r.stderr, r.stderr

        for name, snippet in [
            ("esc_md", "[readme](../README.md)"),
            ("esc_img", "![demo](../assets/demo.svg)"),
            ("esc_pct_link", "[demo](%2e%2e/assets/demo.svg)"),
            ("esc_pct_img", "![demo](%2e%2e/assets/demo.svg)"),
        ]:
            c = td / name
            copy_repo(c)
            p = c / "docs/env.md"
            p.write_text(p.read_text() + f"\n{snippet}\n")
            r = run_build(c)
            assert r.returncode != 0 and "escapes docs/" in r.stderr, (name, r.returncode, r.stderr)

        c_dups = td / "dup_headings"
        copy_repo(c_dups)
        p = c_dups / "docs/env.md"
        p.write_text(p.read_text() + "\n## Foo\n\n## Foo\n\n## Foo-1\n\n[a](#foo) [b](#foo-1) [c](#foo-1-1)\n")
        r = run_build(c_dups)
        assert r.returncode == 0, r.stderr
        html_out = (c_dups / "out/docs/env.html").read_text()
        assert 'id="foo"' in html_out and 'id="foo-1"' in html_out and 'id="foo-1-1"' in html_out

        c_abs = td / "root_abs"
        copy_repo(c_abs)
        p = c_abs / "site/landing.html"
        p.write_text(p.read_text().replace("{{base}}docs/", "/docs/", 1))
        r = run_build(c_abs)
        assert r.returncode != 0 and "/docs/" in r.stderr, r.stderr


if __name__ == "__main__":
    main()
