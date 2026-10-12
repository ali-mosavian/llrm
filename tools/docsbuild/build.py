#!/usr/bin/env python3
"""Build the documentation site.

    python3 tools/docsbuild/build.py --out DIR      write the site to DIR
    python3 tools/docsbuild/build.py --check        build to a temporary directory, print problems

Converts readme.md, agents.md and docs/**/*.md, plus the pass reference that
tools/passdocs.py extracts from the source headers. Exits 1 on a figure that does
not parse, a link that does not resolve, or a construct the converter does not know.
"""
from __future__ import annotations

import argparse
import html
import os
import posixpath
import re
import shutil
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from urllib.parse import unquote

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
REPO = "https://github.com/ali-mosavian/llrm"
GENERATED = "docs/optimizations/reference"
sys.path.insert(0, str(HERE.parent))

import passdocs  # noqa: E402
from docsbuild import mdhtml  # noqa: E402

SCHEME = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.-]*:")


@dataclass
class Page:
    src: str
    text: str
    virtual: bool = False
    res: mdhtml.Result | None = None
    title: str = ""
    ids: set[str] = field(default_factory=set)


def site_path(src: str) -> str:
    """Where the page or asset of repository path `src` lands in the site."""
    if src == "readme.md":
        return "index.html"
    if src == "agents.md":
        return "agents.html"
    rel = src[len("docs/"):] if src.startswith("docs/") else src
    return rel[:-3] + ".html" if rel.endswith(".md") else rel


def collect(root: Path, generated: Path | None) -> dict[str, Page]:
    pages: dict[str, Page] = {}
    for p in ["readme.md", "agents.md", *sorted(x.relative_to(root).as_posix() for x in (root / "docs").rglob("*.md"))]:
        if (root / p).is_file():
            pages[p] = Page(p, (root / p).read_text(encoding="utf-8"))
    if generated:
        for f in sorted(generated.glob("*.md")):
            src = f"{GENERATED}/{f.name}"
            pages[src] = Page(src, f.read_text(encoding="utf-8"), virtual=True)
    return pages


class Site:
    def __init__(self, root: Path, pages: dict[str, Page]) -> None:
        self.root, self.pages = root, pages
        self.assets: dict[str, str] = {}  # repo path -> site path
        self.errors: list[str] = []

    def resolve(self, src: str, href: str) -> tuple[str, str | None]:
        """Rewritten `href` as written in `src`, and an error text if it does not resolve."""
        if SCHEME.match(href):
            return href, None
        path, _, frag = href.partition("#")
        frag = "#" + frag if frag else ""
        if not path:
            return href, None
        target = posixpath.normpath(posixpath.join(posixpath.dirname(src), unquote(path)))
        if target.startswith(".."):
            return href, "link leaves the repository"
        here = posixpath.dirname(site_path(src)) or "."
        if target in self.pages:
            return posixpath.relpath(site_path(target), here) + frag, None
        full = self.root / target
        if full.is_file():
            if target.startswith("docs/") and not target.endswith(".md"):
                self.assets[target] = site_path(target)
                return posixpath.relpath(site_path(target), here) + frag, None
            return f"{REPO}/blob/main/{target}{frag}", None
        if full.is_dir():
            return f"{REPO}/tree/main/{target.rstrip('/')}", None
        return href, f"link does not resolve: {href}"

    def convert(self) -> None:
        for page in self.pages.values():
            page.res = mdhtml.convert(page.text, lambda h, s=page.src: self.resolve(s, h)[0])
            first = next((h for h in page.res.headings if h[0] == 1), None)
            page.title = first[1] if first else page.src
            page.ids = {h[2] for h in page.res.headings}
        for page in self.pages.values():
            r = page.res
            for line, msg in r.figure_errors:
                self.errors.append(f"{page.src}:{line}: figure: {msg}")
            for line, what in r.unrecognised:
                self.errors.append(f"{page.src}:{line}: unconverted construct: {what}")
            for line, href in r.links:
                _, err = self.resolve(page.src, href)
                if err:
                    self.errors.append(f"{page.src}:{line}: {err}")
                    continue
                path, _, frag = href.partition("#")
                if SCHEME.match(href) or not frag:
                    continue
                tgt = page.src if not path else posixpath.normpath(posixpath.join(posixpath.dirname(page.src), unquote(path)))
                if tgt in self.pages and unquote(frag) not in self.pages[tgt].ids and not re.fullmatch(r"L\d+(-L\d+)?", frag):
                    self.errors.append(f"{page.src}:{line}: no heading '#{frag}' in {tgt}")

    # ------------------------------------------------------------ navigation
    def links_of(self, src: str) -> list[str]:
        out: list[str] = []
        for _, href in self.pages[src].res.links:
            if SCHEME.match(href) or not href.partition("#")[0]:
                continue
            t = posixpath.normpath(posixpath.join(posixpath.dirname(src), unquote(href.partition("#")[0])))
            if t in self.pages and t not in out:
                out.append(t)
        return out

    def tree(self) -> list[dict]:
        placed: set[str] = set()
        nodes: dict[str, dict] = {}

        def mk(src):
            placed.add(src)
            nodes[src] = {"src": src, "kids": []}
            return nodes[src]

        def expand(node, depth_rule):
            src = node["src"]
            scope = posixpath.dirname(src) + "/"
            for t in self.links_of(src):
                if t in placed or not t.startswith(scope) or not depth_rule(t):
                    continue
                kid = mk(t)
                node["kids"].append(kid)
                if posixpath.basename(t) == "readme.md":
                    expand(kid, lambda _t: True)

        top = []
        if "readme.md" in self.pages:
            top.append(mk("readme.md"))
        if "docs/readme.md" in self.pages:
            d = mk("docs/readme.md")
            expand(d, lambda t: t.count("/") == 2 and t.endswith("/readme.md"))
            top.append(d)
        if "agents.md" in self.pages:
            top.append(mk("agents.md"))
        # anything a readme did not reach goes under its nearest readme
        for src in self.pages:
            if src in placed or self.pages[src].virtual:
                continue
            d = posixpath.dirname(src)
            while d and f"{d}/readme.md" not in nodes:
                d = posixpath.dirname(d)
            (nodes.get(f"{d}/readme.md") or top[-1])["kids"].append(mk(src))
        ref = [s for s in self.pages if self.pages[s].virtual]
        if ref:
            idx = f"{GENERATED}/index.md"
            parent = nodes.get("docs/optimizations/readme.md") or top[-1]
            head = mk(idx) if idx in self.pages else {"src": ref[0], "kids": []}
            head["kids"] = [mk(s) for s in ref if s != idx]
            parent["kids"].append(head)
        return top

    def nav_html(self, cur: str, top: list[dict]) -> str:
        def has(node):
            return node["src"] == cur or any(has(k) for k in node["kids"])

        def li(node):
            p = self.pages[node["src"]]
            here = posixpath.dirname(site_path(cur)) or "."
            href = posixpath.relpath(site_path(node["src"]), here)
            a = f'<a href="{html.escape(href)}"' + (' aria-current="page"' if node["src"] == cur else "") + f">{html.escape(p.title)}</a>"
            if not node["kids"]:
                return f"<li>{a}</li>"
            op = " open" if has(node) else ""
            return f"<li><details{op}><summary>{a}</summary><ul>{''.join(li(k) for k in node['kids'])}</ul></details></li>"

        return f'<nav aria-label="Documentation"><ul>{"".join(li(n) for n in top)}</ul></nav>'

    # ------------------------------------------------------------ output
    def write(self, out: Path) -> None:
        top = self.tree()
        out.mkdir(parents=True, exist_ok=True)
        (out / ".nojekyll").write_text("")
        (out / "site.css").write_text(CSS, encoding="utf-8")
        for page in self.pages.values():
            dest = out / site_path(page.src)
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(self.page_html(page, top), encoding="utf-8")
        for src, dst in sorted(self.assets.items()):
            (out / dst).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(self.root / src, out / dst)

    def page_html(self, page: Page, top: list[dict]) -> str:
        css = posixpath.relpath("site.css", posixpath.dirname(site_path(page.src)) or ".")
        home = posixpath.relpath("index.html", posixpath.dirname(site_path(page.src)) or ".")
        source = "" if page.virtual else f'<p class="src"><a href="{REPO}/blob/main/{page.src}">Source of this page</a></p>'
        return (
            '<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
            f"<title>{html.escape(page.title)} - llrm</title>\n"
            '<link rel="icon" href="data:,">\n'
            f'<link rel="stylesheet" href="{css}">\n</head>\n<body>\n'
            f'<header><a href="{home}">llrm</a></header>\n<div class="wrap">\n'
            f"{self.nav_html(page.src, top)}\n<main>\n{page.res.html}\n{source}\n</main>\n</div>\n</body>\n</html>\n"
        )


CSS = """:root{--bg:#fff;--fg:#1f2328;--mu:#57606a;--bd:#d0d7de;--sf:#f6f8fa;--ln:#0969da}
@media (prefers-color-scheme:dark){:root{--bg:#14161a;--fg:#e6edf3;--mu:#9da7b3;--bd:#30363d;--sf:#1b1f24;--ln:#58a6ff}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.6 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
a{color:var(--ln)}
header{padding:.6rem 1.2rem;border-bottom:1px solid var(--bd);font-weight:700}
header a{color:var(--fg);text-decoration:none}
.wrap{display:grid;grid-template-columns:18rem minmax(0,1fr);gap:2rem;max-width:80rem;margin:0 auto;padding:0 1.2rem}
nav{position:sticky;top:0;align-self:start;max-height:100vh;overflow:auto;padding:1rem 0;font-size:.9rem}
nav ul{list-style:none;margin:0;padding-left:.9rem}nav>ul{padding-left:0}
nav li{margin:.15rem 0}nav summary{cursor:pointer}
nav a{text-decoration:none}nav a[aria-current]{font-weight:700;color:var(--fg)}
main{min-width:0;padding:1rem 0 4rem;max-width:52rem}
h1,h2,h3,h4{line-height:1.25;margin:1.8em 0 .5em}h1{margin-top:.6em}
code,pre{font:.88em ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
code{background:var(--sf);padding:.1em .3em;border-radius:4px}
pre{background:var(--sf);border:1px solid var(--bd);border-radius:6px;padding:.8rem 1rem;overflow:auto;line-height:1.4}
pre code{background:none;padding:0}
blockquote{margin:1em 0;padding:.1em 1em;border-left:4px solid var(--bd);color:var(--mu)}
.tbl{overflow-x:auto;max-width:100%}table{border-collapse:collapse;margin:1em 0}
th,td{border:1px solid var(--bd);padding:.35rem .7rem;vertical-align:top;text-align:left}th{background:var(--sf)}
img{max-width:100%;height:auto}figure{margin:1.2em 0;text-align:center;overflow-x:auto}figure svg{height:auto}main{min-width:0}
.src{margin-top:3rem;font-size:.85rem;color:var(--mu)}
.figerr{border-color:#cf222e;color:#cf222e}
@media (max-width:52rem){.wrap{grid-template-columns:minmax(0,1fr);gap:0}nav{position:static;max-height:40vh;border-bottom:1px solid var(--bd)}}
"""


def build(root: Path, out: Path, sha: str = "main") -> list[str]:
    """Write the site of repository `root` to `out`; the problems found."""
    with tempfile.TemporaryDirectory() as tmp:
        gen = Path(tmp)
        passdocs.write(gen, root, sha)
        site = Site(root, collect(root, gen))
        site.convert()
        site.write(out)
    return site.errors


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", type=Path, help="write the site here")
    ap.add_argument("--check", action="store_true", help="build to a temporary directory and report problems")
    args = ap.parse_args(argv)
    if not args.out and not args.check:
        ap.error("give --out or --check")
    sha = os.environ.get("GITHUB_SHA") or "main"
    if args.check:
        with tempfile.TemporaryDirectory() as tmp:
            errors = build(ROOT, Path(tmp), sha)
    else:
        errors = build(ROOT, args.out, sha)
    print("\n".join(errors))
    if errors:
        print(f"{len(errors)} problem(s)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
