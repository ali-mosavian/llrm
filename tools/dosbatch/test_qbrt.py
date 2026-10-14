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
PROBES = sorted(path for path in (qbruntime.dosbatch.ROOT / "tests" / "qbrt").glob("*.bas") if not path.name.startswith("gfx_") and ".flat" not in path.name)
EXAMPLES = sorted((qbruntime.dosbatch.ROOT / "examples" / "basic").glob("*.bas"))
# Event trapping, key traps, the absent devices and VARPTR$ are what BC refuses without its /V (test_qbbc.py compares them with BC
# under it, test_qbkeys.py the keys); they have no BCOM45 run to differ from, but they still call runtime entries.
# Launches at once: each runs a share of the programs through the oracle and each target, in a directory of its own.
CHUNKS = 6
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
    return qbruntime.differential_batch(objects, candidates, work / "differential", screens={qbbuild.REAL}, chunks=CHUNKS)


@pytest.fixture(scope="module")
def results(tmp_path_factory):
    """Each program linked and run once against BCOM45 and once on each target of ours."""
    if not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    return differential(SOURCES, tmp_path_factory.mktemp("qbrt"))


# Programs that wait for something outside the runtime.  Strict: one that starts
# to pass must leave this table.
WAITING: dict[str, str] = {}

# (program, target) where ours differs from BCOM45 for a bug that is open: it is expected to differ, and a pass fails the
# test so the entry goes.  Only the flat target has any.
KNOWN: dict[tuple[str, str], str] = {
    ("scores", qbbuild.FLAT): "#1542: a library for BASIC is not supported at a flat target (llrm-nib refuses it by saying so)",
}


def case(name: str):
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


def test_a_run_of_the_programs_mounts_the_tools_as_the_whole_set_would(tmp_path, monkeypatch):
    """Six runs at once: those with no screen job left V: unmounted on the target side only, so the mount lines on the DOS screen differed
    from the oracle's and 58 programs failed on 'row 3: want 80 cells got 0'."""
    monkeypatch.setattr(dosbatch, "DOSBOX", Path("/bin/true"))
    monkeypatch.setattr(dosbatch, "virtual_time_problem", lambda: None)
    job = dosbatch.Job("J000", "exe", tmp_path / "x.exe")
    (tmp_path / "x.exe").write_bytes(b"MZ")
    for forced, mounted in ((None, False), (True, True)):
        work = tmp_path / f"run{mounted}"
        try:
            dosbatch.run([job], work, mount_tools=forced)
        except Exception:  # /bin/true reports no events: only the script it was given matters
            pass
        assert ("mount v" in (work / "jobs.txt").read_text()) is mounted


def test_chunks_hold_every_program_once_in_order():
    names = [f"p{n}" for n in range(100)]
    for chunks in (1, 6, 7, 200):
        parts = qbruntime.chunked(names, chunks)
        assert sum(parts, []) == names and len(parts) <= chunks
