"""Pre-generate what GitHub shows without a build: the pass reference and the figure images.

`plan(root)` computes every file `--sync` writes or deletes; `apply` writes them.

A hand-written figure is a spec in `docs/assets/figures/SLUG.fig`, drawn to `SLUG.png`
beside it; the document holds only `![ALT](../assets/figures/SLUG.png)`. A ```fig fence
(or an older figure block) found in a document is moved to a `.fig` and replaced by the
image. The pass reference is written from the `//!` headers, whose ```fig fences stay in
the source; each becomes an image there. Everything is a pure function of those inputs,
in sorted order.
"""
from __future__ import annotations

import functools
import hashlib
import posixpath
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import passdocs  # noqa: E402
from docsbuild import figblock, figures, png, raster  # noqa: E402

REFERENCE = "docs/optimizations/reference"
FIGURES = "docs/assets/figures"
COMMAND = "python3 tools/docsbuild/build.py --sync"
IMAGE = re.compile(r"!\[[^\]]*\]\([^)\s]*assets/figures/([^/)\s]+)\.png\)")
KEYWORD = "llrm-fig"  # the tEXt keyword that holds a PNG's build key


@dataclass
class Plan:
    writes: dict[str, str | bytes] = field(default_factory=dict)  # changed or new text files, by repository path
    deletes: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    @property
    def drift(self) -> list[str]:
        return sorted([*self.writes, *self.deletes])


def prose(text: str) -> str:
    """`text` without fenced blocks and inline code, where an image reference is not a reference."""
    keep, fenced = [], False
    for line in text.split("\n"):
        if line.startswith(("```", "~~~")):
            fenced = not fenced
        elif not fenced:
            keep.append(re.sub(r"`[^`]*`", "", line))
    return "\n".join(keep)


def alt(fig: figures.Figure) -> str:
    return re.sub(r"\s+", " ", (fig.title or fig.desc.split(":")[0]).replace("[", "(").replace("]", ")")).strip()


def image(slug: str, fig: figures.Figure, from_dir: str) -> str:
    return f"![{alt(fig)}]({posixpath.relpath(f'{FIGURES}/{slug}.png', from_dir or '.')})"


COMMENT = re.compile(r"^<!-- src: (\S+) sha256:([0-9a-f]{64}) -->$")
TOC = re.compile(r"^- \[([^\]]+)\]\(#\1\): ")
STALE = b""  # stands for a PNG that --check found out of date and did not draw


def _digest(files: list[Path]) -> str:
    h = hashlib.sha256()
    for f in files:
        h.update(f.name.encode() + b"\0" + f.read_bytes() + b"\0")
    return h.hexdigest()


@functools.lru_cache(maxsize=1)
def figure_digest() -> str:
    """What a PNG depends on besides its spec: the layout, the drawing, the encoder, the font and the stylesheet."""
    fonts = sorted((HERE / "fonts").glob("*.ttf"))
    return _digest([HERE / n for n in ("figures.py", "raster.py", "png.py", "ttf.py")] + fonts)


@functools.lru_cache(maxsize=1)
def section_digest() -> str:
    """What a reference section depends on besides its header."""
    return _digest([HERE.parent / "passdocs.py", HERE / "sync.py", HERE / "figblock.py"])


def key_of(*parts: str) -> str:
    return hashlib.sha256("\0".join(parts).encode()).hexdigest()


def png_key(path: Path) -> str | None:
    """The build key stored in a PNG, read from its first bytes; None when there is no such file."""
    try:
        with open(path, "rb") as f:
            return png.text_of(f.read(512), KEYWORD)
    except FileNotFoundError:
        return None


@dataclass
class Ctx:
    """What one --sync run knows about the images: those that must exist and those it drew again."""

    root: Path
    render: bool
    force: bool
    keep: set[str] = field(default_factory=set)
    fresh: dict[str, bytes] = field(default_factory=dict)
    errors: list[str] = field(default_factory=list)

    def current(self, slug: str, key: str) -> bool:
        return not self.force and png_key(self.root / FIGURES / f"{slug}.png") == key

    def ensure(self, slug: str, key: str, svg_of) -> None:
        """Make sure `slug`'s PNG carries `key`: keep it when it does, draw it again when not."""
        self.keep.add(slug)
        if not self.current(slug, key):
            self.fresh[slug] = raster.render_png(svg_of(), key) if self.render else STALE


def parse_page(text: str) -> tuple[dict[str, tuple[str, list[str]]], dict[str, str]]:
    """The sections of a committed reference page by source path (digest, lines), and its table of contents."""
    sections: dict[str, tuple[str, list[str]]] = {}
    toc: dict[str, str] = {}
    cur = None
    for line in text.split("\n"):
        m = COMMENT.match(line)
        if m:
            cur = m.group(1)
            sections[cur] = (m.group(2), [line])
        elif line.startswith("## Undocumented"):
            cur = None
        elif cur is not None:
            sections[cur][1].append(line)
        elif (t := TOC.match(line)):
            toc[t.group(1)] = line
    return sections, toc


def make_section(rel: str, lines: list[str], digest: str, ctx: Ctx) -> list[str]:
    """The page lines of one source file, its header figures drawn and replaced by images."""
    body = passdocs.section(rel, lines, "main")
    found, _ = figblock.find("\n".join(body))
    parts = rel.split("/")
    base = f"{parts[parts.index('src') - 1]}-{Path(rel).stem}"
    replace = []
    for n, f in enumerate(found, 1):
        slug = f"{base}-{n}"
        try:
            fig = figures.build(f.spec)
        except figures.FigureError as e:
            ctx.errors.append(f"{rel}: figure {n}: line {e.line}: {e.message}")
            continue
        ctx.ensure(slug, key_of(digest, slug, figure_digest()), lambda fig=fig: fig.svg)
        replace.append((f, image(slug, fig, REFERENCE)))
    for f, img in reversed(replace):
        body[f.start : f.end] = [img]
    return [f"<!-- src: {rel} sha256:{digest} -->", *body]


def reference_pages(root: Path, ctx: Ctx) -> dict[str, str]:
    """The pass, analysis and backend pages and the index, under a banner naming where they come from.

    A section is kept as committed when its header's digest is unchanged and its images are current.
    """
    dirs = [d for _n, _t, d, _k in passdocs.PAGES]
    out: dict[str, str] = {}
    entries = []
    for name, title, directory, kind in passdocs.PAGES:
        path = f"{REFERENCE}/{name}.md"
        committed = (root / path).read_text(encoding="utf-8") if (root / path).is_file() else ""
        old, old_toc = parse_page(committed)
        toc, body, bare = [], [], []
        for src in passdocs.sources(root / directory):
            rel = f"{directory}/{src.name}"
            lines = passdocs.header_of(src.read_text(encoding="utf-8"))
            if not lines:
                bare.append(rel)
                continue
            digest = key_of("\n".join(lines), section_digest())
            kept = old.get(rel)
            slugs = [m.group(1) for m in IMAGE.finditer("\n".join(kept[1]))] if kept else []
            if (
                kept
                and kept[0] == digest
                and src.stem in old_toc
                and all(ctx.current(s, key_of(digest, s, figure_digest())) for s in slugs)
            ):
                ctx.keep.update(slugs)
                body += kept[1]
                toc.append(old_toc[src.stem])
            else:
                body += make_section(rel, lines, digest, ctx)
                toc.append(passdocs.toc_line(src.stem, lines))
        banner = (
            "> Generated by `tools/docsbuild/build.py --sync` from the `//!` headers of "
            f"`{directory}`; edit the headers, not this file.\n\n"
        )
        out[path] = banner + passdocs.assemble(title, directory, toc, body, bare, "main")
        entries.append(passdocs.index_line(name, title, len(toc), len(bare)))
    where = ", ".join(f"`{d}`" for d in dirs)
    banner = f"> Generated by `tools/docsbuild/build.py --sync` from the `//!` headers of {where}; edit the headers, not this file.\n\n"
    out[f"{REFERENCE}/index.md"] = banner + passdocs.index_text(entries)
    return out


def slug_base(path: str) -> str:
    return path[:-3].replace("/", "-")


def markdown_files(root: Path, generated: dict[str, str]) -> list[str]:
    on_disk = [p.relative_to(root).as_posix() for p in (root / "docs").rglob("*.md")]
    return sorted({*(p for p in ("readme.md", "agents.md") if (root / p).is_file()), *on_disk, *generated})


def png_refs(text: str) -> str:
    """Image references to a figure's SVG become references to its PNG, outside code."""
    keep, fenced = [], False
    for line in text.split("\n"):
        if line.startswith(("```", "~~~")):
            fenced = not fenced
        elif not fenced:
            parts = line.split("`")
            line = "`".join(
                re.sub(r"(!\[[^\]]*\]\([^)\s]*assets/figures/[^/)\s]+)\.svg\)", r"\1.png)", p) if i % 2 == 0 else p
                for i, p in enumerate(parts)
            )
        keep.append(line)
    return "\n".join(keep)


def migrate(path: str, text: str, taken: set[str]) -> tuple[str, dict[str, str], list[str]]:
    """`text` with each fence or figure block moved out: the new specs by slug, and problems."""
    found, errs = figblock.find(text)
    problems = [f"{path}:{line}: {msg}" for line, msg in errs]
    lines = text.split("\n")
    specs: dict[str, str] = {}
    n = 0
    plan_: list[tuple[figblock.Fig, str, str]] = []
    for f in found:
        n += 1
        while f"{slug_base(path)}-{n}" in taken:
            n += 1
        slug = f"{slug_base(path)}-{n}"
        try:
            fig = figures.build(f.spec)
        except figures.FigureError as e:
            problems.append(f"{path}:{f.spec_line + e.line - 1}: figure: {e.message}")
            continue
        taken.add(slug)
        specs[slug] = f.spec
        plan_.append((f, slug, image(slug, fig, posixpath.dirname(path))))
    for f, _slug, img in reversed(plan_):
        lines[f.start : f.end] = [img]
    return "\n".join(lines), specs, problems


def plan(root: Path, render: bool = True, force: bool = False) -> Plan:
    """What --sync would write and delete; with `render` false (--check) PNGs are compared by key, never drawn."""
    out = Plan()
    ctx = Ctx(root, render, force)
    generated = reference_pages(root, ctx)
    folder = root / FIGURES
    specs = {f.stem: f.read_text(encoding="utf-8").rstrip("\n") for f in sorted(folder.glob("*.fig"))} if folder.is_dir() else {}
    taken = set(specs) | ctx.keep
    texts: dict[str, str] = {}
    for path in markdown_files(root, generated):
        if path in generated:
            texts[path] = generated[path]
            old = (root / path).read_text(encoding="utf-8") if (root / path).is_file() else None
        else:
            old = (root / path).read_text(encoding="utf-8")
            text, new, problems = migrate(path, png_refs(old), taken)
            out.errors += problems
            specs.update(new)
            texts[path] = text
        if texts[path] != old:
            out.writes[path] = texts[path]
    refs: dict[str, str] = {}
    for path, text in texts.items():
        for m in IMAGE.finditer(prose(text)):
            refs.setdefault(m.group(1), path)
    for slug, spec in sorted(specs.items()):
        file = f"{FIGURES}/{slug}.fig"
        try:
            ctx.ensure(slug, key_of(spec, figure_digest()), lambda spec=spec: figures.build(spec).svg)
        except figures.FigureError as e:
            out.errors.append(f"{file}:{e.line}: figure: {e.message}")
        if not (root / file).is_file() or (root / file).read_text(encoding="utf-8") != spec + "\n":
            out.writes[file] = spec + "\n"
        if slug not in refs:
            out.errors.append(f"{file}: not referenced by any document")
    for slug, path in sorted(refs.items()):
        if slug not in ctx.keep:
            out.errors.append(f"{path}: {FIGURES}/{slug}.png has no {slug}.fig")
    out.errors += ctx.errors
    for slug, data in sorted(ctx.fresh.items()):
        file = root / FIGURES / f"{slug}.png"
        if data == STALE or not file.is_file() or file.read_bytes() != data:
            out.writes[f"{FIGURES}/{slug}.png"] = data
    out.deletes = sorted(
        p.relative_to(root).as_posix()
        for p in sorted([*folder.glob("*.svg"), *folder.glob("*.png")])
        if p.suffix == ".svg" or p.stem not in ctx.keep
    ) if folder.is_dir() else []
    return out


def fence_errors(root: Path) -> list[str]:
    """A figure fence or block in a committed document is output of an older --sync, not source."""
    out = []
    for path in markdown_files(root, {}):
        found, _ = figblock.find((root / path).read_text(encoding="utf-8"))
        out += [f"{path}:{f.start + 1}: figure fence in a document; run {COMMAND}" for f in found]
    return out


def apply(root: Path, p: Plan) -> None:
    for path, content in p.writes.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        if isinstance(content, bytes):
            (root / path).write_bytes(content)
        else:
            (root / path).write_text(content, encoding="utf-8")
    for path in p.deletes:
        (root / path).unlink()


def drift_message(p: Plan) -> str:
    files = "\n".join(f"  {f}" for f in p.drift)
    return f"generated docs are out of date; run {COMMAND}\n{files}"
