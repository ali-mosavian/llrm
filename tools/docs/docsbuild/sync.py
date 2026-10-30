"""Write one Markdown page per documented source file, from its `//!` header.

`plan(root)` computes every file `--sync` writes or deletes; `apply` writes them.

For each directory of `passdocs.PAGES` (passes, analyses, backend phases) the pages go to
`docs/optimizations/NAME/FILE.md`, with a `readme.md` index beside them. A source file with no
header block is listed in the index as undocumented and has no page. Subdirectories of a source
directory are not read; a module's header belongs to its own `mod.rs`/`NAME.rs` file.

A page holds a banner, the file stem as its title and the header with its `# ` headings demoted
one level and its rustdoc links turned into Markdown links. Each page carries a digest of its
header, of this tool and of what its own links resolve to, so adding a file changes only the pages
that link its name; a page whose digest is unchanged is left as committed.
"""
from __future__ import annotations

import functools
import hashlib
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import passdocs  # noqa: E402

OUT = "docs/optimizations"
COMMAND = "python3 tools/docs/docsbuild/build.py --sync"
URL = f"{passdocs.REPO}/blob/main"
SECTIONS = ("Runs", "Used by", "Provides")
KEY = re.compile(r"^<!-- src: (\S+) sha256:([0-9a-f]{64}) -->$")
LINK = re.compile(r"\[`([^`\]]+)`\](?:\[[^\]]*\]|\(([\w:]+)\))?(?![(\[])")


@dataclass
class Plan:
    writes: dict[str, str] = field(default_factory=dict)  # changed or new files, by repository path
    deletes: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    @property
    def drift(self) -> list[str]:
        return sorted([*self.writes, *self.deletes])


def key_of(*parts: str) -> str:
    return hashlib.sha256("\0".join(parts).encode()).hexdigest()


# What a page depends on besides its header and the names it links to: the layout the pages are written in. It was the hash of this file and
# passdocs.py, so that moving or editing either rewrote every page (417 files by one digest). Bump it when the output format changes;
# test_the_page_format_is_pinned fails until you do.
FORMAT = "13c5cd8c082ae91fa3eeb4eb3de20ea1724b4ef50d18b17ea9b3fbd5891b1565"


def tool_digest() -> str:
    return FORMAT


# ---------------------------------------------------------------- links


@dataclass
class Names:
    """What a rustdoc link can name: the pages, and the source files of the crates."""

    pages: dict[str, str]  # file stem -> page directory name (passes, analyses, backend)
    files: dict[str, list[str]]  # file stem -> repository paths of `crates/**/STEM.rs`



def names_of(root: Path, documented: dict[str, str]) -> Names:
    files: dict[str, list[str]] = {}
    crates = root / "crates"
    for p in sorted(crates.rglob("*.rs")) if crates.is_dir() else []:
        rel = p.relative_to(root).as_posix()
        if "/target/" not in rel:
            files.setdefault(p.stem, []).append(rel)
    return Names(documented, files)


def target_of(path: str, here: str, names: Names) -> str | None:
    """Where a rustdoc path points from a page in directory `here`: a sibling page, else the source on GitHub."""
    segs = [s for s in path.split("::") if s not in ("crate", "self", "super")]
    segs = [s for s in segs if not s.startswith("llrm_")] or segs
    for s in segs:
        if s in names.pages:
            d = names.pages[s]
            return f"{s}.md" if d == here else f"../{d}/{s}.md"
    hits = names.files.get(segs[0], []) if segs else []
    return f"{URL}/{hits[0]}" if len(hits) == 1 else None


def relink(lines: list[str], here: str, names: Names) -> list[str]:
    """`lines` with rustdoc links as Markdown links, or plain code when nothing is there to link to."""
    out, fenced = [], False

    def one(m: re.Match) -> str:
        text, path = m.group(1), m.group(2) or m.group(1)
        dest = target_of(path, here, names) if re.fullmatch(r"\w+(::\w+)*", path) else None
        return f"[`{text}`]({dest})" if dest else f"`{text}`"

    for line in lines:
        if line.lstrip().startswith("```"):
            fenced = not fenced
        elif not fenced:
            line = LINK.sub(one, line)
        out.append(line)
    return out


def demote(lines: list[str]) -> list[str]:
    """Each heading one level deeper, outside code fences."""
    out, fenced = [], False
    for line in lines:
        if line.lstrip().startswith("```"):
            fenced = not fenced
        elif not fenced and line.startswith("#"):
            line = "#" + line
        out.append(line)
    return out


# ---------------------------------------------------------------- pages


def banner(rel: str) -> str:
    return f"> Generated from the `//!` header of [`{rel}`]({URL}/{rel}); edit the header, not this file."


def page_text(rel: str, lines: list[str], here: str, names: Names, key: str) -> str:
    body = demote(relink(lines, here, names))
    return "\n".join([banner(rel), f"<!-- src: {rel} sha256:{key} -->", "", f"# {Path(rel).stem}", "", *body, ""])


def committed_key(text: str) -> str | None:
    for line in text.split("\n")[:3]:
        m = KEY.match(line)
        if m:
            return m.group(2)
    return None


def escape_cell(s: str) -> str:
    return s.replace("|", "\\|")


def index_text(title: str, directory: str, kind: str, rows: list[tuple], lead: str) -> str:
    done = sum(1 for r in rows if r[1] is not None)
    head = [
        f"> Generated from the `//!` headers of `{directory}`; edit the headers, not this file.",
        "",
        f"# {title}",
        "",
        f"From the `//!` headers of `{directory}`: {done} documented, {len(rows) - done} undocumented.",
        "",
    ]
    if lead:
        head += [lead, ""]
    table = [f"| Name | Summary | {kind} | Page |", "|---|---|---|---|"]
    for stem, summary, line, link in rows:
        if summary is None:
            table.append(f"| {stem} | undocumented | | [source]({link}) |")
        else:
            table.append(f"| {stem} | {escape_cell(summary)} | {escape_cell(line)} | [{stem}]({link}) |")
    return "\n".join(head + table) + "\n"


def plan(root: Path, force: bool = False) -> Plan:
    """What --sync would write and delete."""
    out = Plan()
    headers: dict[str, dict[str, list[str]]] = {}
    bare: dict[str, list[str]] = {}
    for name, _title, directory, _kind in passdocs.PAGES:
        headers[name], bare[name] = {}, []
        users = passdocs.users_of(root / directory, root)
        for src in passdocs.sources(root / directory):
            lines = passdocs.header_of(src.read_text(encoding="utf-8"))
            if lines:
                headers[name][src.stem] = passdocs.with_users(lines, users.get(src.stem, []))
            else:
                bare[name].append(src.stem)
    names = names_of(root, {stem: n for n, hs in headers.items() for stem in hs})
    wanted: set[str] = set()
    for name, title, directory, kind in passdocs.PAGES:
        rows: list[tuple] = []
        lead = ""
        for stem, lines in sorted(headers[name].items()):
            path = f"{OUT}/{name}/{stem}.md"
            wanted.add(path)
            key = key_of("\n".join(lines), tool_digest(), "\n".join(relink(lines, name, names)))
            old = (root / path).read_text(encoding="utf-8") if (root / path).is_file() else None
            if force or old is None or committed_key(old) != key:
                text = page_text(f"{directory}/{stem}.rs", lines, name, names, key)
                if text != old:
                    out.writes[path] = text
            if stem == "pipeline" and name == "passes":
                lead = f"The order the passes run in: [pipeline](pipeline.md), {passdocs.summary(lines)}"
                continue
            line = passdocs.section_line(lines, SECTIONS)
            cell = relink([line], name, names)[0] if line else ""
            rows.append((stem, passdocs.summary(lines), cell, f"{stem}.md"))
        rows += [(stem, None, "", f"{URL}/{directory}/{stem}.rs") for stem in bare[name]]
        rows.sort(key=lambda r: r[0])
        ipath = f"{OUT}/{name}/readme.md"
        wanted.add(ipath)
        itext = index_text(title, directory, kind, rows, lead)
        if not (root / ipath).is_file() or (root / ipath).read_text(encoding="utf-8") != itext:
            out.writes[ipath] = itext
    for name, *_ in passdocs.PAGES:
        folder = root / OUT / name
        for stale in sorted(folder.glob("*.md")) if folder.is_dir() else []:
            rel = stale.relative_to(root).as_posix()
            if rel not in wanted:
                out.deletes.append(rel)
    return out


def missing_example(root: Path) -> list[str]:
    """`file: no # Example` for each documented source file whose header lacks the heading."""
    found = []
    for _name, _title, directory, _kind in passdocs.PAGES:
        for src in passdocs.sources(root / directory):
            lines = passdocs.header_of(src.read_text(encoding="utf-8"))
            if lines and "Example" not in passdocs.sections(lines):
                found.append(f"{directory}/{src.name}: no # Example")
    return found


def apply(root: Path, p: Plan) -> None:
    for path, content in p.writes.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(content, encoding="utf-8")
    for path in p.deletes:
        (root / path).unlink()


def drift_message(p: Plan) -> str:
    files = "\n".join(f"  {f}" for f in p.drift)
    return f"generated docs are out of date; run {COMMAND}\n{files}"
