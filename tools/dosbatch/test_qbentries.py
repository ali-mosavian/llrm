"""Every runtime entry a program can call is called by a probe: a prototype that drifts from what llrm-qb passes breaks only the
programs that call it, so an entry no probe calls would break silently.  Needs no BCOM45: the objects are compiled, not run."""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qbbuild  # noqa: E402
import qbruntime  # noqa: E402

PROBES = sorted(path for path in (qbruntime.dosbatch.ROOT / "tests" / "qbrt").glob("*.bas") if not path.name.startswith("gfx_") and ".flat" not in path.name)
ALL_SOURCES = {path.stem: path for path in [*PROBES, *qbruntime.milestone_sources(), *sorted((qbruntime.dosbatch.ROOT / "examples" / "basic").glob("*.bas"))]}


def symbols(obj: Path, kind: int) -> set[str]:
    """The names an OMF object defines (PUBDEF, 0x90) or references (EXTDEF, 0x8C)."""
    data, at, found = obj.read_bytes(), 0, set()
    while at < len(data):
        record, size = data[at], data[at + 1] | data[at + 2] << 8
        body = data[at + 3 : at + 2 + size]
        at += 3 + size
        if record != kind:
            continue
        i = 0
        if kind == 0x90:  # the group and segment indexes, and a frame number where there is no segment
            i = 2 + (2 if body[1] == 0 else 0)
        while i < len(body):
            length = body[i]
            found.add(body[i + 1 : i + 1 + length].decode("latin-1"))
            i += 1 + length + (3 if kind == 0x90 else 1)
    return found


# Reached only with the QB_POC_CHK environment variable set (semantic.rs): a proof of concept, not a statement. Debt `qb-poc-chk-entries`.
POC_ONLY = {"B$CHKEXP", "B$CHKEX2", "B$CHKLOG"}


def test_every_runtime_entry_is_called_by_a_probe_under_llrm(tmp_path_factory):
    """A prototype that drifts from what llrm-qb passes breaks only the programs that call it, so an
    entry no probe calls would break silently."""
    work = tmp_path_factory.mktemp("qbrt-entries")
    objects = qbbuild.runtime16(work / "rt")
    exported = set()
    # What a program can call is what the frontend's runtime table names; the rest of the B$ names are
    # the target's helpers and handlers.
    table = (qbruntime.dosbatch.ROOT / "crates/frontends/qb/runtime.toml").read_text()
    entries = set(re.findall(r"B\$\w+", table))
    for obj in objects.values():
        # The *USED names are markers the compiler refers to, not routines to call.
        exported |= {name for name in symbols(obj, 0x90) if name in entries and not name.endswith("USED")}
    called = set()
    pictures = sorted(path for path in (qbruntime.dosbatch.ROOT / "tests" / "qbrt").glob("gfx_*.bas") if ".flat" not in path.name)
    probes = sorted((qbruntime.dosbatch.ROOT / "tests" / "qbentries").glob("*.bas"))  # compile-only: one statement each, no BCOM45 run
    for name, source in {**ALL_SOURCES, **{path.stem: path for path in pictures}, **{path.stem: path for path in probes}}.items():
        obj = work / f"{name}.obj"
        assert qbruntime.compile_basic(source, obj, "llrm") is None
        called |= symbols(obj, 0x8C)
        for library in qbruntime.linked_objects(source, work, "llrm"):
            called |= symbols(library, 0x8C)
    assert exported
    assert sorted(exported - called - POC_ONLY) == []
