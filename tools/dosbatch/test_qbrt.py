"""The QB runtime against BCOM45: every probe in tests/qbrt and every milestone program, raw bytes."""

from __future__ import annotations

import re
import sys
from pathlib import Path

import pytest

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
    ("bc_sound", qbbuild.FLAT): "#1482: the program ends at the second error under ON ERROR, with the 1Ch vector still taken",
    ("rnd_val", qbbuild.FLAT): "#1512: RND after RANDOMIZE n",
    ("color_masks", qbbuild.FLAT): "#1538: Out of DATA",
    ("fre_lprint", qbbuild.FLAT): "#1539: FRE",
    ("read_real", qbbuild.FLAT): "#1540: the last digit of a DOUBLE",
    ("real_print_sweep", qbbuild.FLAT): "#1540: the last digit of a DOUBLE",
    ("real_print_wide", qbbuild.FLAT): "#1540: the last digit of a DOUBLE",
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


def test_every_runtime_entry_is_called_by_a_probe_under_llrm(tmp_path_factory):
    """A prototype that drifts from what llrm-qb passes breaks only the programs that call it, so an
    entry no probe calls would break silently."""
    if not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    work = tmp_path_factory.mktemp("qbrt-entries")
    objects = qbbuild.runtime16(work / "rt")
    exported = set()
    # What a program can call is what the frontend's runtime table names; the rest of the B$ names are
    # the target's helpers and handlers.
    table = (qbruntime.dosbatch.ROOT / "crates/frontends/llrm-qbruntime/src/runtime.toml").read_text()
    entries = set(re.findall(r"B\$\w+", table))
    for obj in objects.values():
        # The *USED names are markers the compiler refers to, not routines to call.
        exported |= {name for name in symbols(obj, 0x90) if name in entries and not name.endswith("USED")}
    called = set()
    pictures = sorted((qbruntime.dosbatch.ROOT / "tests" / "qbrt").glob("gfx_*.bas"))
    for name, source in {**ALL_SOURCES, **{path.stem: path for path in pictures}}.items():
        obj = work / f"{name}.obj"
        assert qbruntime.compile_basic(source, obj, "llrm") is None
        called |= symbols(obj, 0x8C)
        for library in qbruntime.linked_objects(source, work, "llrm"):
            called |= symbols(library, 0x8C)
    assert exported
    assert sorted(exported - called) == []
