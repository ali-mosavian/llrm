"""The QB runtime against BCOM45: every probe in tests/qbrt and every milestone program, raw bytes."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

# BCOM45 (BC, LINK and its runtime) is the oracle these tests compare against: the gate drops the file where QB45 is not installed.
REQUIRES = ["qb45"]

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbbuild  # noqa: E402
import qbruntime  # noqa: E402

# The graphics probes are compared as pictures (test_qbgfx.py).
PROBES = sorted(path for path in (qbruntime.dosbatch.ROOT / "tests" / "qbrt").glob("*.bas") if not path.name.startswith("gfx_"))
EXAMPLES = sorted((qbruntime.dosbatch.ROOT / "examples" / "basic").glob("*.bas"))
# Event trapping, key traps, the absent devices and VARPTR$ are what BC refuses without its /V (test_qbbc.py compares them with BC
# under it, test_qbkeys.py the keys); they have no BCOM45 run to differ from, but they still call runtime entries.
LLRM_ONLY = ("events_", "keys_", "llrm_", "bc_nodevices")
ALL_SOURCES = {path.stem: path for path in [*PROBES, *qbruntime.milestone_sources(), *EXAMPLES]}
SOURCES = {name: path for name, path in ALL_SOURCES.items() if not name.startswith(LLRM_ONLY)}


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
    return qbruntime.differential_batch(objects, candidates, work / "differential", screens={qbbuild.REAL})


@pytest.fixture(scope="module")
def results(tmp_path_factory):
    """Each program linked and run once against BCOM45 and once on each target of ours."""
    if not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    return differential(SOURCES, tmp_path_factory.mktemp("qbrt"))


# Programs that wait for something outside the runtime.  Strict: one that starts
# to pass must leave this table.
WAITING: dict[str, str] = {}

# Fail or pass with the runtime's code layout, not with what the runtime does: a kernel padded by 40 bytes with nops
# makes color_pairs pass, 16 does not (#1488).  Not strict: the issue stays open until the cause is found.
LAYOUT_DEPENDENT: dict[str, str] = {
    "color_pairs": "#1488: -O2 reads an array back as 0 after a handled-error loop; passes when the runtime's layout moves",
    "color_ranges": "#1488: the same bug; the screen compare differs",
}


# (program, target) where ours differs from BCOM45 for a bug that is open: it is expected to differ, and a pass fails the
# test so the entry goes.  Only the flat target has any.
KNOWN: dict[tuple[str, str], str] = {
    ("rnd_val", qbbuild.FLAT): "#1512: RND after RANDOMIZE n",
    ("fre_lprint", qbbuild.FLAT): "#1539: FRE",
    ("environ_command", qbbuild.FLAT): "#1541: ENVIRON$ and COMMAND$ do not link",
    ("scores", qbbuild.FLAT): "#1542: the ' link: library does not compile for the flat runtime",
}


def case(name: str):
    if reason := LAYOUT_DEPENDENT.get(name):
        return pytest.param(name, marks=pytest.mark.xfail(strict=False, reason=reason))
    reason = WAITING.get(name)
    return pytest.param(name, marks=pytest.mark.xfail(strict=True, reason=reason)) if reason else name


@pytest.mark.parametrize("name", [case(name) for name in sorted(SOURCES)])
def test_program_matches_bcom45_byte_for_byte(results, name: str):
    for target, result in results[name].items():
        assert result.reference.status == "ok"
        differs = result.candidate.status != "ok" or result.difference != "" or result.screen_difference != ""
        if reason := KNOWN.get((name, target)):
            assert differs, f"{target} now matches BCOM45: remove it from KNOWN ({reason})"
            continue
        assert result.candidate.status == "ok", f"{target}: {result.candidate.detail}"
        assert result.difference == "", f"{target}: {result.difference}"
        assert result.screen_difference == "", f"{target}: {result.screen_difference}"


@pytest.fixture(scope="module")
def demos(tmp_path_factory):
    """The Microsoft demo programs, built and run the same way."""
    found, reason = qbruntime.demo_sources()
    if not found or not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip(reason or "QB45_DIR is unavailable")
    return differential({source.stem.upper(): source for source in found.values()}, tmp_path_factory.mktemp("demos"))


@pytest.mark.parametrize("name", ["NIBBLES", "GORILLA"])
def test_demo_introduction_matches_bcom45(demos, name: str):
    """Both draw the same introduction and wait for a key (NIBBLES's random sparkles aside)."""
    for target, result in demos[name].items():
        assert result.candidate.status in ("ok", "stopped"), f"{target}: {result.candidate.detail}"
        assert result.reference.status in ("ok", "stopped")
        assert result.screen_difference == "", target
