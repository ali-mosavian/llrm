"""The source trees every measurement reads, named in one place with the hash of each.

A tool that read $QCPORT as "the tree with d_faces.c" got QCport's own src, which lacks the game headers d_faces includes: the compile stopped after
8 M instructions, perf counted it, and the exit code was ignored. Each tool now asks for a corpus by name; a tree whose hash differs from the
registered one is refused, and the refusal names the registered corpus it matches, if any.

    from corpora import resolve, header
    src = resolve("dfaces", os.environ.get("DFACES_SRC") or os.environ.get("QCPORT"), args.allow_other_corpus)
    print(header("dfaces", src))

The hash is the first 12 hex digits of sha256 over `sha256sum ./path` lines of every *.c and *.h (any case) under the tree, in byte order of the path.
No other tool reads DFACES_SRC, QCPORT, QCPORT_INC or QCPORT_SRC: tools/test_corpora.py greps for it.
`python3 tools/corpora.py list|path NAME|hash DIR|check`.
"""

from __future__ import annotations

import hashlib
import os
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class Corpus:
    env: str | None
    default: str
    purpose: str
    hash: str | None = None  # None: a tree in this repository, pinned by git
    record: bool = True


REGISTRY = {
    "dfaces": Corpus("DFACES_SRC", "~/scratch/qbq-llrm/src", "the d_faces compile-time pin: render/d_faces.c with the game headers it includes; every scoreboard row", "53880fe70961"),
    "qcport": Corpus("QCPORT", "~/scratch/qcport/src", "the gate's QCport: 65 modules", "5539820ae87e"),
    "qcport-inc": Corpus("QCPORT_INC", "~/scratch/qctc/inc", "the Borland headers QCport builds with", "71005c701bf0"),
    "qcport-gcc": Corpus(
        "QCPORT_GCC",
        "~/scratch/qcport-gcc",
        "triage tree, not a corpus of record: a patched copy of qcport (21 files dated 2026-10-09, creator unknown): record-size static asserts rewritten "
        "`? 1 : -1` to `? 1 : 1`, and game/triggers.c:354 is a real expression changed by a too-wide sed, which changes the program's meaning",
        "52d87710a4fc",
        record=False,
    ),
    "bench": Corpus(None, "bench", "the bench programs"),
    "tests-run": Corpus(None, "tests/run", "the run tests"),
}


def tree_hash(path: str | Path) -> str:
    base = Path(path).expanduser()
    files = sorted((p.relative_to(base).as_posix() for p in base.rglob("*") if p.is_file() and p.suffix.lower() in (".c", ".h")), key=lambda r: r.encode())
    listing = "".join(f"{hashlib.sha256((base / rel).read_bytes()).hexdigest()}  ./{rel}\n" for rel in files)
    return hashlib.sha256(listing.encode()).hexdigest()[:12]


def _matches(digest: str) -> str | None:
    return next((name for name, one in REGISTRY.items() if one.hash == digest), None)


def _chosen(name: str, override: str | Path | None) -> str | Path | None:
    """The named tree: `override`, else the corpus's variable. `dfaces` also accepts the legacy $QCPORT when that is the d_faces tree (it was
    the only variable for it); QCport's own src in $QCPORT is not it and the default is used."""
    one = REGISTRY[name]
    if override:
        return override
    chosen = os.environ.get(one.env) if one.env else None
    if not chosen and name == "dfaces":
        legacy = os.environ.get("QCPORT")
        if legacy and Path(legacy).expanduser().is_dir() and tree_hash(legacy) == one.hash:
            return legacy
    return chosen


def optional(name: str, allow_other: bool = False) -> Path | None:
    """The tree of corpus `name` when its variable is set or its default exists, else None (a tool that skips what is not there)."""
    one = REGISTRY[name]
    if _chosen(name, None) or Path(one.default).expanduser().is_dir():
        return resolve(name, None, allow_other)
    return None


def announce(name: str, path: str | Path) -> None:
    print(header(name, path), file=sys.stderr)


def resolve(name: str, override: str | Path | None = None, allow_other: bool = False) -> Path:
    """The tree of corpus `name`: `override`, else its environment variable, else its default. A pinned tree with another hash is a SystemExit
    unless `allow_other` (the tool's --allow-other-corpus)."""
    one = REGISTRY[name]
    chosen = _chosen(name, override)
    path = (Path(chosen) if chosen else (ROOT / one.default if one.hash is None else Path(one.default))).expanduser()
    if not path.is_dir():
        raise SystemExit(f"corpus {name}: {path} is not a directory")
    if one.hash is not None and not allow_other:
        digest = tree_hash(path)
        if digest != one.hash:
            other = _matches(digest)
            raise SystemExit(
                f"corpus {name}: {path} is tree {digest}, not the registered {one.hash}"
                + (f"; that tree is the registered corpus `{other}`" if other else "")
                + "; name it with --allow-other-corpus if that is the one to measure"
            )
    return path


def header(name: str, path: str | Path) -> str:
    one = REGISTRY[name]
    return f"corpus {name}: {path} tree {tree_hash(path)}" + ("" if one.record else " (triage tree, not a corpus of record)")


def main() -> int:
    argv = sys.argv[1:]
    if argv == ["list"]:
        for name, one in REGISTRY.items():
            print(f"{name}\t{one.hash or 'git'}\t{one.env or '-'}\t{one.default}\t{one.purpose}")
        return 0
    if len(argv) == 2 and argv[0] == "path":
        print(resolve(argv[1]))
        return 0
    if len(argv) == 2 and argv[0] == "hash":
        print(tree_hash(argv[1]))
        return 0
    if argv == ["check"]:
        bad = 0
        for name, one in REGISTRY.items():
            if one.hash is None:
                continue
            try:
                print(header(name, resolve(name)))
            except SystemExit as stop:
                bad += 1
                print(stop)
        return 1 if bad else 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
