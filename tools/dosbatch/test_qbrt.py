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

from qbrt_programs import BC_KNOWN, real_bc_text, not_real_bc  # noqa: E402
from qbrt_programs import ALL_SOURCES, CHUNKS, EXAMPLES, INTERACTIVE, KNOWN, LLRM_ONLY, PROBES, SOURCES, differential  # noqa: E402,F401


@pytest.fixture(scope="module")
def results(tmp_path_factory):
    """Each program linked and run once against BCOM45 and once on each target of ours."""
    if not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    return differential(SOURCES, tmp_path_factory.mktemp("qbrt"))


# Programs that wait for something outside the runtime.  Strict: one that starts
# to pass must leave this table.
WAITING: dict[str, str] = {}

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
def games(tmp_path_factory):
    """The interactive examples, built and run for the oracle and each target: they never end, so the run is stopped at the limit."""
    if not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip("QB45_DIR is unavailable")
    return differential({name: ALL_SOURCES[name] for name in INTERACTIVE}, tmp_path_factory.mktemp("games"))


@pytest.mark.parametrize("name", INTERACTIVE)
def test_an_interactive_game_draws_what_bcom45_draws(games, name: str):
    """Compiled and run for the oracle and every target; stopped at the limit, the screens must agree (as the Microsoft demos' introductions do)."""
    for target, result in games[name].items():
        assert result.candidate.status in ("ok", "stopped"), f"{target}: {result.candidate.detail}"
        assert result.reference.status in ("ok", "stopped")
        assert result.screen_difference == "", target


def test_no_game_that_waits_for_a_key_is_compared_byte_for_byte():
    """gorillas and nibbles (examples/basic) never end: both arms ran to the 120 s limit and the reference was "stopped", not "ok" (gate run 3)."""
    assert not {"gorillas", "nibbles"} & set(SOURCES)


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


CHECKED_AGAINST_BC = sorted(name for name, path in SOURCES.items() if real_bc_text(path) is not None)


@pytest.mark.parametrize("name", CHECKED_AGAINST_BC)
def test_float_output_is_real_bcs_text(results, name: str):
    """The differential's reference is our compiler on BC's runtime, so a LOG/EXP/^ digit wrong in our expansion is wrong in both arms and passes it (real_print_wide printed 9 lines
    unlike BC with reference == candidate). The program's output is also compared with BC's own text."""
    stored = real_bc_text(SOURCES[name])
    problems = []
    for target, result in results[name].items():
        if why := not_real_bc(stored, result.reference.text, result.candidate.text) if target == qbbuild.REAL else None:
            problems.append(f"{target}: {why}")
    if name in BC_KNOWN:
        assert problems, f"{name} now matches real BC: remove it from BC_KNOWN ({BC_KNOWN[name]})"
        return
    assert not problems, "; ".join(problems)
