import argparse
import html
import json
import posixpath
import re
import shutil
import sys
from collections import deque
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

from markdown_it import MarkdownIt
from mdit_py_plugins.tasklists import tasklists_plugin

ROOT = Path(__file__).resolve().parent.parent
DOCS_DIR = ROOT / "docs"
SITE_DIR = ROOT / "site"
ASSETS_DIR = SITE_DIR / "assets"
TEMPLATES_DIR = SITE_DIR / "templates"
REDIRECTS_FILE = SITE_DIR / "redirects.json"
CANONICAL_ORIGIN = "https://ethanhamilthon.github.io"
EXTERNAL_SCHEMES = ("http://", "https://", "mailto:", "data:")
BUILD_MARKER = ".tuse-site-build"
PROTECTED_DIRS = (
    DOCS_DIR,
    SITE_DIR,
    ROOT / ".git",
    ROOT / ".github",
    ROOT / "crates",
    ROOT / "release",
    ROOT / "plugins",
)


def normalize_base(raw: str) -> str:
    s = raw.strip()
    if not s or s == "/":
        return "/"
    return "/" + s.strip("/") + "/"


def validate_out_dir(raw: str) -> Path:
    raw_path = Path(raw)
    if raw_path.is_symlink():
        raise ValueError(f"refusing to write through symlink --out '{raw}'")
    out_dir = raw_path.resolve()
    if out_dir == out_dir.parent or out_dir == ROOT or ROOT.is_relative_to(out_dir):
        raise ValueError(f"refusing unsafe --out '{raw}' (resolves to '{out_dir}')")
    for protected in PROTECTED_DIRS:
        if out_dir == protected or out_dir.is_relative_to(protected):
            raise ValueError(f"refusing protected --out '{raw}' (inside '{protected}')")
    if out_dir.exists():
        if not out_dir.is_dir():
            raise ValueError(f"--out '{raw}' exists and is not a directory")
        has_entries = any(out_dir.iterdir())
        if has_entries and not (out_dir / BUILD_MARKER).is_file():
            raise ValueError(
                f"refusing to overwrite non-empty directory '{out_dir}' without {BUILD_MARKER}"
            )
    return out_dir


def github_slug(text: str, used: set[str]) -> str:
    lowered = text.strip().lower()
    kept = "".join(ch for ch in lowered if ch.isalnum() or ch in (" ", "-", "_"))
    base = kept.replace(" ", "-")
    candidate = base
    n = 1
    while candidate in used:
        candidate = f"{base}-{n}"
        n += 1
    used.add(candidate)
    return candidate


def inline_plain_text(token) -> str:
    if not token.children:
        return token.content or ""
    parts = []
    for child in token.children:
        if child.type in ("text", "code_inline"):
            parts.append(child.content)
        elif child.type in ("softbreak", "hardbreak"):
            parts.append(" ")
    return "".join(parts)


def rewrite_md_dest(dest: str, rel_md: str, is_link: bool, errors: list[str]) -> str:
    if not dest or dest.startswith("#") or dest.startswith(EXTERNAL_SCHEMES):
        return dest
    parsed = urlsplit(dest)
    if parsed.scheme:
        return dest
    raw_path = parsed.path
    decoded_path = unquote(raw_path)
    frag = f"#{parsed.fragment}" if parsed.fragment else ""
    kind = "link" if is_link else "image"
    if raw_path.startswith("/") or decoded_path.startswith("/"):
        errors.append(f"docs/{rel_md}: {kind} '{dest}' must stay relative inside docs/")
        return dest
    src_dir = posixpath.dirname(rel_md)
    joined = posixpath.normpath(posixpath.join(src_dir, decoded_path))
    if joined == ".." or joined.startswith("../"):
        errors.append(f"docs/{rel_md}: {kind} '{dest}' escapes docs/")
        return dest
    if is_link and decoded_path.endswith(".md"):
        if joined == "index.md":
            rel_dir = posixpath.relpath(".", src_dir or ".")
            target = "./" if rel_dir == "." else rel_dir.rstrip("/") + "/"
            return target + frag
        if joined.endswith("/index.md"):
            rel_dir = posixpath.relpath(posixpath.dirname(joined), src_dir or ".")
            target = "./" if rel_dir == "." else rel_dir.rstrip("/") + "/"
            return target + frag
        if raw_path.endswith(".md"):
            return raw_path[:-3] + ".html" + frag
        return decoded_path[:-3] + ".html" + frag
    return dest


def make_markdown() -> MarkdownIt:
    md = MarkdownIt("gfm-like", {"html": False, "linkify": False, "typographer": False})
    md.use(tasklists_plugin)
    return md


def render_markdown(md: MarkdownIt, text: str, rel_md: str, errors: list[str]) -> tuple[str, str]:
    tokens = md.parse(text)
    used_ids: set[str] = set()
    title = ""
    i = 0
    while i < len(tokens):
        tok = tokens[i]
        if tok.type == "heading_open" and i + 1 < len(tokens):
            heading_text = inline_plain_text(tokens[i + 1]).strip()
            if tok.tag == "h1" and not title:
                title = heading_text
            tok.attrSet("id", github_slug(heading_text, used_ids))
        if tok.children:
            for child in tok.children:
                if child.type == "link_open":
                    href = child.attrGet("href") or ""
                    child.attrSet("href", rewrite_md_dest(href, rel_md, True, errors))
                elif child.type == "image":
                    src = child.attrGet("src") or ""
                    child.attrSet("src", rewrite_md_dest(src, rel_md, False, errors))
        i += 1
    rendered = md.renderer.render(tokens, md.options, {})
    rendered = rendered.replace("<table>", '<div class="table-wrap"><table>')
    rendered = rendered.replace("</table>", "</table></div>")
    return rendered, title


def parse_sidebar(index_text: str) -> list[tuple[str, list[tuple[str, str]]]]:
    groups: list[tuple[str, list[tuple[str, str]]]] = []
    cur_title = ""
    cur_items: list[tuple[str, str]] = []
    for line in index_text.splitlines():
        m_head = re.match(r"^##\s+(.+)$", line)
        if m_head:
            if cur_title:
                groups.append((cur_title, cur_items))
            cur_title = m_head.group(1).strip()
            cur_items = []
            continue
        m_item = re.match(r"^-\s+\[(.+?)\]\(([^)]+)\)", line)
        if m_item and cur_title:
            label = m_item.group(1).replace("`", "")
            href = m_item.group(2).strip()
            if href.endswith(".md"):
                href = "./" if href == "index.md" else href[:-3] + ".html"
            cur_items.append((label, href))
    if cur_title:
        groups.append((cur_title, cur_items))
    return groups


def render_sidebar(
    groups: list[tuple[str, list[tuple[str, str]]]], base: str, current_rel: str
) -> str:
    idx_cur = ' aria-current="page"' if current_rel == "index.html" else ""
    parts = [
        '<div class="sidebar-group">',
        '<p class="sidebar-title">Overview</p>',
        "<ul>",
        f'<li><a href="{base}docs/"{idx_cur}>Documentation Index</a></li>',
        "</ul></div>",
    ]
    for title, items in groups:
        parts.append('<div class="sidebar-group">')
        parts.append(f'<p class="sidebar-title">{html.escape(title)}</p>')
        parts.append("<ul>")
        for label, href in items:
            full = f"{base}docs/" if href == "./" else f"{base}docs/{href}"
            cur = ' aria-current="page"' if href == current_rel else ""
            parts.append(f'<li><a href="{html.escape(full)}"{cur}>{html.escape(label)}</a></li>')
        parts.append("</ul></div>")
    return "\n".join(parts)


def fill_template(tmpl: str, values: dict[str, str]) -> str:
    out = tmpl
    for k, v in values.items():
        out = out.replace("{{" + k + "}}", v)
    return out


def write_redirect(out_dir: Path, src_rel: str, dst_rel: str, base: str) -> None:
    target = f"{base}{dst_rel}"
    canonical = f"{CANONICAL_ORIGIN}{target}"
    page = (
        "<!doctype html>\n"
        '<html lang="en">\n<head>\n'
        '  <meta charset="utf-8">\n'
        f'  <meta http-equiv="refresh" content="0; url={html.escape(target)}">\n'
        f'  <link rel="canonical" href="{html.escape(canonical)}">\n'
        "  <title>Redirecting…</title>\n"
        "</head>\n<body>\n"
        f'  <p>Redirecting to <a href="{html.escape(target)}">{html.escape(target)}</a>.</p>\n'
        "</body>\n</html>\n"
    )
    dest = (out_dir / src_rel).resolve()
    if not dest.is_relative_to(out_dir) or dest.exists():
        raise ValueError(f"unsafe redirect source '{src_rel}'")
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(page, encoding="utf-8")


def build_site(base: str, out_dir: Path) -> tuple[list[str], set[str], dict[str, str]]:
    errors: list[str] = []
    if out_dir.exists() and (out_dir / BUILD_MARKER).is_file():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / BUILD_MARKER).write_text("tuse-site\n", encoding="utf-8")

    if ASSETS_DIR.exists():
        shutil.copytree(ASSETS_DIR, out_dir / "assets")

    base_tmpl = (TEMPLATES_DIR / "base.html").read_text(encoding="utf-8")
    doc_tmpl = (TEMPLATES_DIR / "doc.html").read_text(encoding="utf-8")
    landing_body = fill_template(
        (SITE_DIR / "landing.html").read_text(encoding="utf-8"),
        {"base": base},
    )

    landing_html = fill_template(
        base_tmpl,
        {
            "base": base,
            "title": "Tuse — minimal AI coding agent for the terminal",
            "description": "Minimal AI coding agent for the terminal written in Rust, centered on a single lua tool in a persistent Lua 5.4 VM.",
            "og_url": f"{CANONICAL_ORIGIN}{base}",
            "nav_home_attr": ' aria-current="page"',
            "nav_docs_attr": "",
            "body": landing_body,
        },
    )
    (out_dir / "index.html").write_text(landing_html, encoding="utf-8")

    md = make_markdown()
    index_text = (DOCS_DIR / "index.md").read_text(encoding="utf-8")
    sidebar_groups = parse_sidebar(index_text)
    rendered_docs: set[str] = set()

    for path in sorted(DOCS_DIR.rglob("*")):
        if path.is_dir():
            continue
        rel = path.relative_to(DOCS_DIR).as_posix()
        if path.suffix != ".md":
            dst = out_dir / "docs" / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, dst)
            continue
        text = path.read_text(encoding="utf-8")
        body_html, heading = render_markdown(md, text, rel, errors)
        out_rel = rel[:-3] + ".html"
        rendered_docs.add(f"docs/{out_rel}")
        is_planned = rel.startswith("planned/")
        planned_banner = ""
        if is_planned:
            planned_banner = (
                '<div class="planned-banner">'
                '<span class="badge-planned">Planned</span>'
                f'<span><a href="{base}docs/">Docs</a> / <a href="{base}docs/#planned-changes">Planned Changes</a> / {html.escape(heading or out_rel)}</span>'
                "</div>"
            )
        doc_body = fill_template(
            doc_tmpl,
            {
                "sidebar": render_sidebar(sidebar_groups, base, out_rel),
                "planned_banner": planned_banner,
                "content": body_html,
            },
        )
        page_url = "docs/" if out_rel == "index.html" else f"docs/{out_rel}"
        full_html = fill_template(
            base_tmpl,
            {
                "base": base,
                "title": f"{heading} — Tuse Docs" if heading else "Tuse Docs",
                "description": f"Tuse documentation: {heading}" if heading else "Tuse documentation",
                "og_url": f"{CANONICAL_ORIGIN}{base}{page_url}",
                "nav_home_attr": "",
                "nav_docs_attr": ' aria-current="page"',
                "body": doc_body,
            },
        )
        dst = out_dir / "docs" / out_rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_text(full_html, encoding="utf-8")

    redirects: dict[str, str] = {}
    if REDIRECTS_FILE.exists():
        redirects = json.loads(REDIRECTS_FILE.read_text(encoding="utf-8"))
        for src_rel, dst_rel in sorted(redirects.items()):
            write_redirect(out_dir, src_rel, dst_rel, base)

    return errors, rendered_docs, redirects


class HtmlInspector(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.ids: set[str] = set()
        self.dup_ids: list[str] = []
        self.links: list[tuple[str, str]] = []
        self.assets: list[tuple[str, str]] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attr_map = {k: (v or "") for k, v in attrs}
        if "id" in attr_map and attr_map["id"]:
            ident = attr_map["id"]
            if ident in self.ids:
                self.dup_ids.append(ident)
            self.ids.add(ident)
        if tag == "a" and "href" in attr_map:
            self.links.append((tag, attr_map["href"]))
        elif tag == "link" and "href" in attr_map:
            if attr_map.get("rel") != "canonical":
                self.assets.append((tag, attr_map["href"]))
        elif tag in ("script", "img") and "src" in attr_map:
            self.assets.append((tag, attr_map["src"]))
        elif tag == "source" and "srcset" in attr_map:
            for item in attr_map["srcset"].split(","):
                part = item.strip().split()[0] if item.strip() else ""
                if part:
                    self.assets.append((tag, part))
        elif tag == "meta":
            if attr_map.get("property") == "og:image" and attr_map.get("content"):
                self.assets.append(("meta[og:image]", attr_map["content"]))
            if attr_map.get("http-equiv", "").lower() == "refresh":
                m = re.search(r"url=(.+)$", attr_map.get("content", ""), re.IGNORECASE)
                if m:
                    self.links.append(("meta[refresh]", m.group(1).strip()))


def resolve_built_url(
    url: str, page_rel: str, base: str, out_dir: Path, errors: list[str], kind: str
) -> tuple[str | None, str]:
    if not url or url.startswith(EXTERNAL_SCHEMES) or url.startswith("//"):
        return None, ""
    parsed = urlsplit(url)
    if parsed.scheme:
        return None, ""
    path_part = unquote(parsed.path)
    frag = unquote(parsed.fragment)
    if not path_part:
        return page_rel, frag
    if path_part.startswith("/"):
        if base != "/" and not path_part.startswith(base):
            errors.append(
                f"{page_rel}: {kind} '{url}' is root-absolute without base '{base}'"
            )
            return None, frag
        rel_from_root = path_part[len(base) :] if path_part.startswith(base) else path_part.lstrip("/")
        norm = posixpath.normpath(rel_from_root) if rel_from_root else ""
    else:
        src_dir = posixpath.dirname(page_rel)
        joined = posixpath.join(src_dir, path_part)
        norm = posixpath.normpath(joined)
    if norm == ".":
        norm = ""
    if norm == ".." or norm.startswith("../"):
        errors.append(f"{page_rel}: {kind} '{url}' escapes site root")
        return None, frag
    if path_part.endswith("/") or norm == "":
        candidate = posixpath.join(norm, "index.html") if norm else "index.html"
    else:
        candidate = norm
        if (out_dir / candidate).is_dir():
            candidate = posixpath.join(candidate, "index.html")
    if not (out_dir / candidate).is_file():
        errors.append(f"{page_rel}: {kind} '{url}' -> missing target '{candidate}'")
        return None, frag
    return candidate, frag


def check_site(
    base: str,
    out_dir: Path,
    build_errors: list[str],
    rendered_docs: set[str],
    redirects: dict[str, str],
) -> list[str]:
    errors = list(build_errors)
    for tmpl in [SITE_DIR / "landing.html", *sorted(TEMPLATES_DIR.glob("*.html"))]:
        text = tmpl.read_text(encoding="utf-8")
        for m in re.finditer(r'(?:href|src|srcset|content)=["\'](/(?:docs|assets)/[^"\']*)["\']', text):
            errors.append(
                f"{tmpl.relative_to(ROOT)}: unprefixed root-absolute URL '{m.group(1)}' (use {{{{base}}}})"
            )

    inspectors: dict[str, HtmlInspector] = {}
    for html_file in sorted(out_dir.rglob("*.html")):
        rel = html_file.relative_to(out_dir).as_posix()
        parser = HtmlInspector()
        parser.feed(html_file.read_text(encoding="utf-8"))
        for dup in parser.dup_ids:
            errors.append(f"{rel}: duplicate id '{dup}'")
        inspectors[rel] = parser

    graph: dict[str, set[str]] = {rel: set() for rel in inspectors}
    for rel, parser in inspectors.items():
        for tag, href in parser.links:
            target, frag = resolve_built_url(href, rel, base, out_dir, errors, f"<{tag}>")
            if target is None:
                continue
            graph[rel].add(target)
            if frag:
                check_target = target
                if check_target in redirects:
                    dst = redirects[check_target]
                    check_target = "index.html" if not dst else (f"{dst}index.html" if dst.endswith("/") else dst)
                target_ids = inspectors.get(check_target)
                if target_ids is None or frag not in target_ids.ids:
                    errors.append(
                        f"{rel}: link '{href}' references missing anchor '#{frag}' in '{check_target}'"
                    )
        for tag, src in parser.assets:
            resolve_built_url(src, rel, base, out_dir, errors, f"<{tag}>")

    docs_index = "docs/index.html"
    visited: set[str] = set()
    queue: deque[str] = deque([docs_index]) if docs_index in inspectors else deque()
    while queue:
        cur = queue.popleft()
        if cur in visited:
            continue
        visited.add(cur)
        for nxt in graph.get(cur, ()):
            if nxt.startswith("docs/") and nxt not in visited:
                queue.append(nxt)

    for doc_page in sorted(rendered_docs):
        if doc_page not in visited:
            errors.append(f"{doc_page}: orphan page not reachable from {docs_index}")

    return errors


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="/tuse/")
    ap.add_argument("--out", default="dist")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    base = normalize_base(args.base)
    try:
        out_dir = validate_out_dir(args.out)
    except ValueError as exc:
        print(f"site build error: {exc}", file=sys.stderr)
        return 1
    errors, rendered_docs, redirects = build_site(base, out_dir)
    if args.check:
        errors = check_site(base, out_dir, errors, rendered_docs, redirects)
    if errors:
        for err in errors:
            print(f"site check error: {err}", file=sys.stderr)
        return 1
    print(f"Built {len(rendered_docs)} doc pages + landing page into {out_dir} (base={base})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
