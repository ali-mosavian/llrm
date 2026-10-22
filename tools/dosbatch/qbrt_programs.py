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
ALL_SOURCES = {path.stem: path for path in [*PROBES, *qbruntime.milestone_sources(), *EXAMPLES]}
SOURCES = {name: path for name, path in ALL_SOURCES.items() if not name.startswith(LLRM_ONLY)}

# (program, target) where ours differs from BCOM45 for a bug that is open: it is expected to differ, and a pass fails the
# test so the entry goes.  Only the flat target has any.
KNOWN: dict[tuple[str, str], str] = {
    ("scores", qbbuild.FLAT): "#1542: a library for BASIC is not supported at a flat target (llrm-nib refuses it by saying so)",
}


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
