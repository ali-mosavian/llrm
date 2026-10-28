"""Check the mermaid figures of the documents without mermaid: structure only, in pure Python.

`fences(text)` finds the ```mermaid fences of a Markdown text; `check(body)` lists what is wrong with
one; `check_tree(root)` does it for every tracked Markdown file and every `//!` header, and rejects a
legacy ```fig fence. `render_errors(root)` also draws each figure with mermaid-cli when `$MMDC` or
`mmdc` on `$PATH` is there, and says so when it is not.
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import passdocs  # noqa: E402
from docsbuild import tomermaid as tm  # noqa: E402

TYPES = ("flowchart", "block-beta", "gantt", "sequenceDiagram")
KNOWN_CLASSES = set(tm.CLASSES) | set(tm.BLOCK_CLASSES) | {"cell"}
ENTITY = re.compile(r"#\w+;")
QUOTED = re.compile(r'"[^"]*"')
TAGS = re.compile(r"</?(?:b|small|br/?)>")
EDGE_LABEL = re.compile(r"\|[^|]*\|")
EDGE_TEXT = re.compile(r"(?:--|==|-\.)\s+.+?\s*(?:-->|==>|\.->|---|===|\.-)")
ARROW = re.compile(r"\s*(?:<?-->|<?-\.->|<?==>|---|-\.-|===)\s*(?:\|\s*\|)?\s*")
FENCE = re.compile(r"^(\s*)(`{3,}|~{3,})\s*(\S*)\s*$")


def fences(text: str) -> list[tuple[int, str, str]]:
    """(line, language, body) of each fenced block of `text`, the line 1-based at the opening fence."""
    lines = text.split("\n")
    out, i = [], 0
    while i < len(lines):
        m = FENCE.match(lines[i])
        if not m:
            i += 1
            continue
        close = re.compile(rf"^{re.escape(m.group(2)[0])}{{{len(m.group(2))},}}\s*$")
        j = i + 1
        while j < len(lines) and not close.match(lines[j].strip()):
            j += 1
        out.append((i + 1, m.group(3), "\n".join(lines[i + 1 : j])))
        i = j + 1
    return out


def diagram_type(body: str) -> str | None:
    for line in body.split("\n")[1:]:
        if line.strip():
            word = line.split()[0]
            return word if word in TYPES else None
    return None


def check(body: str) -> list[str]:
    """What is wrong with the mermaid text `body`, as messages."""
    lines = body.split("\n")
    kind = diagram_type(body)
    if kind is None:
        return [f"second line must start with one of {', '.join(TYPES)}"]
    errs = []
    if lines[0] != tm.INITS[kind]:
        errs.append(f"first line must be the init line of a {kind} figure (tomermaid.INITS)")
    rest = lines[1:]
    errs += _syntax(rest)
    errs += _classes(rest)
    if kind == "flowchart":
        errs += _flowchart(rest)
    elif kind == "block-beta":
        errs += _block(rest)
    elif kind == "sequenceDiagram":
        errs += _sequence(rest)
    return errs


def _syntax(lines: list[str]) -> list[str]:
    errs = []
    for line in lines:
        if line.count('"') % 2:
            errs.append(f"unbalanced quote: {line.strip()}")
            continue
        flat = QUOTED.sub('""', line)
        for open_, close in ("[]", "()", "{}"):
            if flat.count(open_) != flat.count(close):
                errs.append(f"unbalanced {open_}{close}: {line.strip()}")
        if flat.count("|") % 2:
            errs.append(f"unbalanced |: {line.strip()}")
        for label in QUOTED.findall(line):
            inner = TAGS.sub("", ENTITY.sub("", label[1:-1]))
            if "<" in inner or ">" in inner:
                errs.append(f"raw angle bracket in a label: {label}")
        if ";" in ENTITY.sub("", flat):
            errs.append(f"';' outside a quoted label: {line.strip()}")
    return errs


def _classes(lines: list[str]) -> list[str]:
    defined = {m.group(1) for line in lines if (m := re.match(r"\s*classDef\s+(\w+)", line))}
    used = set()
    for line in lines:
        used.update(re.findall(r":::(\w+)", line))
        m = re.match(r"\s*class\s+[\w,]+\s+(\w+)\s*$", line)
        if m:
            used.add(m.group(1))
    return [f"class '{c}' is defined nowhere (tomermaid.CLASSES)" for c in sorted(used) if c not in defined | KNOWN_CLASSES]


def _flowchart(lines: list[str]) -> list[str]:
    declared, edges = set(), []
    for line in lines:
        s = line.strip()
        m = re.match(r"subgraph\s+(\w+)", s)
        if m:
            declared.add(m.group(1))
            continue
        if re.match(r"(classDef|class|style|linkStyle|end|direction|flowchart)\b", s) or not s:
            continue
        flat = EDGE_TEXT.sub(" --> ", EDGE_LABEL.sub("", QUOTED.sub('""', s)))
        for decl in re.finditer(r"(\w+)(?=\s*(?:\[|\(|\{|:::))", flat):
            declared.add(decl.group(1))
        flat = re.sub(r":::\w+", "", flat)
        while True:
            new = re.sub(r"\[[^\[\]]*\]|\([^()]*\)|\{[^{}]*\}", "", flat)
            if new == flat:
                break
            flat = new
        parts = [p.strip() for p in ARROW.split(flat)]
        if len(parts) > 1:
            edges.append((s, [x for p in parts for x in p.split("&") if x.strip()]))
    errs = []
    for s, ids in edges:
        for x in ids:
            x = x.strip()
            if x and x not in declared:
                errs.append(f"edge names '{x}', which is not a node or subgraph: {s}")
    return errs


def _block(lines: list[str]) -> list[str]:
    cols = next((int(m.group(1)) for line in lines if (m := re.match(r"\s*columns\s+(\d+)", line))), None)
    if cols is None:
        return ["block-beta figure has no 'columns N' line"]
    errs = []
    for line in lines:
        s = line.strip()
        if not s or re.match(r"(columns|classDef|class|style)\b", s):
            continue
        flat = QUOTED.sub('""', s)
        total = 0
        for tok in re.finditer(r"space(?::(\d+))?|\w+\[\"\"\](?::(\d+))?", flat):
            span = int(tok.group(1) or tok.group(2) or 1)
            if span > cols:
                errs.append(f"block spans {span} of {cols} columns: {s}")
            total += span
        if total > cols:
            errs.append(f"row of {total} blocks in {cols} columns: {s}")
    return errs


def _sequence(lines: list[str]) -> list[str]:
    declared = {m.group(1) for line in lines if (m := re.match(r"\s*participant\s+(\w+)", line))}
    errs = []
    for line in lines:
        m = re.match(r"\s*(\w+)\s*-{1,2}>>\s*(\w+)\s*:", line) or re.match(r"\s*Note over (\w+)()\s*:", line)
        if m:
            errs += [f"sequence line names '{x}', which is not a participant: {line.strip()}" for x in m.groups() if x and x not in declared]
    return errs


# ---------------------------------------------------------------- the tree


def tracked(root: Path, pattern: str) -> list[Path]:
    """Tracked files matching `pattern` (a glob), by git; every matching file when `root` is no repository."""
    if (root / ".git").exists():
        out = subprocess.run(["git", "ls-files", pattern], cwd=root, capture_output=True, text=True, check=True).stdout
        return [root / p for p in out.splitlines() if (root / p).is_file()]
    return sorted(p for p in root.rglob(pattern.split("/")[-1]) if "target" not in p.parts and ".git" not in p.parts)


def documents(root: Path) -> list[tuple[str, str]]:
    """(label, text) of each tracked Markdown file and each `//!` header, the header with its prefix stripped."""
    docs = [(p.relative_to(root).as_posix(), p.read_text(encoding="utf-8")) for p in tracked(root, "*.md")]
    for p in tracked(root, "*.rs"):
        head = passdocs.header_of(p.read_text(encoding="utf-8", errors="replace"))
        if any(line.lstrip().startswith("```") for line in head):
            docs.append((p.relative_to(root).as_posix(), "\n".join(head)))
    return docs


def figures_of(root: Path) -> list[tuple[str, str]]:
    """(label, mermaid text) of every figure in the tree, labelled `path:line`."""
    return [(f"{label}:{line}", body) for label, text in documents(root) for line, lang, body in fences(text) if lang == "mermaid"]


GENERATED = re.compile(r"docs/optimizations/(?:passes|analyses|backend)/")


def fix_style(text: str) -> str:
    """`text` with the init line of each of its mermaid fences made the shared one (`tomermaid.normalize_init`)."""
    lines = text.split("\n")
    for line, lang, body in reversed(fences(text)):
        if lang == "mermaid":
            n = len(body.split("\n"))
            lines[line : line + n] = tm.normalize_init(body).split("\n")
    return "\n".join(lines)


def fix_style_tree(root: Path) -> list[str]:
    """Apply `fix_style` to each tracked Markdown file of docs/ that is not generated; the paths it changed."""
    changed = []
    for p in tracked(root, "docs/**/*.md"):
        rel = p.relative_to(root).as_posix()
        if not rel.startswith("docs/") or GENERATED.match(rel):
            continue
        old = p.read_text(encoding="utf-8")
        new = fix_style(old)
        if new != old:
            p.write_text(new, encoding="utf-8")
            changed.append(rel)
    return changed


def check_tree(root: Path) -> list[str]:
    errs = []
    for label, text in documents(root):
        for line, lang, body in fences(text):
            if lang == "fig":
                errs.append(f"{label}:{line}: legacy ```fig fence; convert it with tools/docs/docsbuild/mdfigs.py")
            elif lang == "mermaid":
                errs += [f"{label}:{line}: {e}" for e in check(body)]
    return errs


def find_mmdc() -> str | None:
    return os.environ.get("MMDC") or shutil.which("mmdc")


def render_errors(root: Path) -> list[str]:
    """Draw every figure with mermaid-cli; a figure it cannot parse is an error. [] and a message when it is absent."""
    mmdc = find_mmdc()
    if not mmdc:
        print("--render skipped: mmdc is not on PATH and $MMDC is not set", file=sys.stderr)
        return []
    errs = []
    with tempfile.TemporaryDirectory() as tmp:
        for n, (label, body) in enumerate(figures_of(root)):
            src = Path(tmp) / f"f{n}.mmd"
            src.write_text(body, encoding="utf-8")
            cfg = os.environ.get("PUPPETEER_CONFIG")
            cmd = [mmdc, "-i", str(src), "-o", str(src.with_suffix(".png")), *(["-p", cfg] if cfg else [])]
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
            if r.returncode or not src.with_suffix(".png").is_file():
                errs.append(f"{label}: mmdc: {(r.stderr or r.stdout).strip().splitlines()[-1:] or ['failed']}")
    return errs
