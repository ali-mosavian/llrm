"""The programs of tests/qbrt and the differential that runs them against BCOM45, without pytest: test_qbrt.py (the tests) and qbrt_run.py (the
command line) both read them."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402,F401
import qbbuild  # noqa: E402
import qbruntime  # noqa: E402

# The graphics probes are compared as pictures (test_qbgfx.py).
PROBES = sorted(path for path in (qbruntime.dosbatch.ROOT / "tests" / "qbrt").glob("*.bas") if not path.name.startswith("gfx_") and ".flat" not in path.name)
EXAMPLES = sorted((qbruntime.dosbatch.ROOT / "examples" / "basic").glob("*.bas"))
# Event trapping, key traps, the absent devices and VARPTR$ are what BC refuses without its /V (test_qbbc.py compares them with BC
# under it, test_qbkeys.py the keys); they have no BCOM45 run to differ from, but they still call runtime entries.
# Launches at once: each runs a share of the programs through the oracle and each target, in a directory of its own.
CHUNKS = 6
LLRM_ONLY = ("events_", "keys_", "llrm_", "bc_nodevices")
# Games that wait for a key forever: the oracle and the candidate both run to the 120 s limit, so the byte-for-byte test (reference "ok")
# fails on them; test_demo_introduction_matches_bcom45 compares the introductions.
INTERACTIVE = ("gorillas", "nibbles")
ALL_SOURCES = {path.stem: path for path in [*PROBES, *qbruntime.milestone_sources(), *EXAMPLES]}
SOURCES = {name: path for name, path in ALL_SOURCES.items() if not name.startswith(LLRM_ONLY) and name not in INTERACTIVE}

# (program, target) where ours differs from BCOM45 for a bug that is open: it is expected to differ, and a pass fails the
# test so the entry goes.
KNOWN: dict[tuple[str, str], str] = {
    ("scores", qbbuild.FLAT): "#1542: a library for BASIC is not supported at a flat target (llrm-nib refuses it by saying so)",
}


# Programs whose output is not real BC's text today, strict: one that starts to match must leave this table.
BC_KNOWN: dict[str, str] = {
    "real_print_wide": "#3506 (7a99c56e9): the range checks spill the operand as a double, ^ prints 15 digits; goes with its revert or fix-forward",
}


def real_bc_text(source: Path) -> list[str] | None:
    """Real BC's output lines for a program, where it is checked in: `NAME.bcout` beside the source (taken from BC.EXE + LINK + BCOM45; a bench program's `.out` is for
    its own input, so it is not used). The differential's reference is our compiler on the MS runtime, not BC's code, so a digit our expansions get wrong is the same in both arms."""
    found = source.with_suffix(".bcout")
    return lines_of(found.read_text(errors="replace")) if found.exists() else None


def lines_of(text: str) -> list[str]:
    return [one.rstrip() for one in text.replace("\r", "").split("\n") if one.strip()]


def not_real_bc(
    stored: list[str],
    reference: str,
    candidate: str,
) -> str | None:
    """Why a run is not real BC's text, or None. A reference and a candidate that agree with each other and not with BC are said so: that is the case the byte-for-byte
    comparison alone passes."""
    ref, cand = lines_of(reference), lines_of(candidate)
    if ref == cand and ref != stored:
        return f"reference and candidate agree but are not real BC's text (first difference: {first_difference(stored, ref)})"
    if cand != stored:
        return f"candidate is not real BC's text ({first_difference(stored, cand)})"
    return None


def first_difference(want: list[str], got: list[str]) -> str:
    for at in range(max(len(want), len(got))):
        w, g = (want[at] if at < len(want) else "<end>"), (got[at] if at < len(got) else "<end>")
        if w != g:
            return f"line {at + 1}: BC {w!r}, got {g!r}"
    return ""


def differential(sources: dict[str, Path], work: Path) -> dict[str, dict]:
    """Each program as BCOM45 builds it (the oracle) and ours at each target, run and compared."""
    objects = {}
    candidates: dict[str, dict] = {target: {} for target in qbbuild.TARGETS}
    runtime = qbbuild.runtimes(work)
    for name, source in sources.items():
        obj = work / f"{name}.qb45.obj"
        error = qbruntime.compile_basic(source, obj, "qb45")
        assert error is None, error
        objects[name] = (obj, *qbruntime.linked_objects(source, work, "qb45"))
        for target, built in qbbuild.build(source, work / f"b_{name}", runtime, tolerant=True).items():
            candidates[target][name] = built  # or why it did not build: that program's test says so
    # a flat program runs behind DOS/32A's banner: the raw bytes are compared there, the screens in real mode
    return qbruntime.differential_batch(objects, candidates, work / "differential", screens={qbbuild.REAL}, chunks=CHUNKS)
