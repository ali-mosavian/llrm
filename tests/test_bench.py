"""Benchmark failures must not turn into flattering speed measurements."""
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import bench


@pytest.mark.parametrize("compile_log,link_log,finished,accepted", [
    ("1 Severe Error(s)", "Microsoft (R) Segmented Executable Linker\n" * 2, True, False),
    ("", "Microsoft (R) Segmented Executable Linker\n" * 2, True, False),
    ("0 Severe Error(s)", "Microsoft (R) Segmented Executable Linker\nerror L2029: unresolved external", True, False),
    ("0 Severe Error(s)", "", True, False),
    ("0 Severe Error(s)", "Microsoft (R) Segmented Executable Linker\n" * 2, False, False),
    ("    0 Severe  Error(s)\r\n", "Microsoft (R) Segmented Executable Linker\n" * 2, True, True),
])
def test_failed_build_artifacts_are_not_benchmarked(monkeypatch, tmp_path, compile_log, link_log, finished, accepted):
    """BC can emit an OBJ after severe errors; LINK can emit an EXE with unresolved calls."""
    monkeypatch.setattr(bench, "BUILD", tmp_path)
    monkeypatch.setattr(bench, "CONFIGS", {"test": SimpleNamespace(
        available=True, mount=tmp_path, bc="BC", link="LINK", runtime="RUNTIME")})
    monkeypatch.setattr(bench, "switches_for", lambda *args: "")
    def launch(work, *args, **kwargs):
        for name in ("NBODY.OBJ", "BASE.EXE", "OPT.EXE"):
            (work / name).write_bytes(b"artifact despite error")
        return SimpleNamespace(finished=finished, timed_out=not finished)
    monkeypatch.setattr(bench, "launch", launch)
    monkeypatch.setattr(bench, "read_dos", lambda work, name: compile_log if name == "BC.OUT" else link_log)
    if accepted:
        assert all(path.is_file() for path in bench.build("test", transform=lambda data: data))
    else:
        with pytest.raises(SystemExit):
            bench.build("test", transform=lambda data: data)


@pytest.mark.parametrize("output", ["PX0= 1\nTICKS= 0\nDONE", "PX0= 1\nTICKS= 10"])
def test_invalid_timer_or_incomplete_answer_is_not_timed(monkeypatch, output):
    """Optimized NBODY printed TICKS=0 after its timer's high-byte clear was miscompiled."""
    monkeypatch.setattr(bench, "launch", lambda *a, **kw: SimpleNamespace(finished=True, timed_out=False))
    monkeypatch.setattr(bench, "read_dos", lambda *a: output)
    with pytest.raises(SystemExit):
        bench.run("v-g3", Path("OPT.EXE"), 100, 1)


def test_timeout_does_not_reuse_an_old_reading(monkeypatch):
    """A leftover valid OUT file cannot make a timed-out run successful."""
    monkeypatch.setattr(bench, "launch", lambda *a, **kw: SimpleNamespace(finished=False, timed_out=True))
    monkeypatch.setattr(bench, "read_dos", lambda *a: "PX0= 1\nTICKS= 10\nDONE")
    with pytest.raises(SystemExit):
        bench.run("v-g3", Path("OPT.EXE"), 100, 1)


def test_refusal_is_not_benchmarked_as_optimization(monkeypatch):
    """NBODY's unchanged fallback was previously eligible for an 'optimized' timing."""
    monkeypatch.setattr(bench.wholeseg, "emitted", lambda *a, **kw: SimpleNamespace(
        outcome=bench.wholeseg.Emission.REFUSED, reason="missing byte move", data=b"original"))
    with pytest.raises(SystemExit, match="missing byte move"):
        bench.optimized(b"original", False, "386")


def test_wrong_answer_is_not_a_speedup(monkeypatch):
    """A faster but wrong floating benchmark previously supplied a quoted speedup."""
    monkeypatch.setattr(bench, "launch", lambda *a, **kw: SimpleNamespace(finished=True, timed_out=False))
    monkeypatch.setattr(bench, "read_dos", lambda *a: "PX0= -2147483648\nTICKS= 10\nDONE")
    with pytest.raises(SystemExit, match="answer mismatch"):
        bench.run("v-g3", Path("OPT.EXE"), 100, 1, expected=("PX0= 1", "DONE"))


def test_each_repetition_is_checked_and_both_outputs_are_preserved(monkeypatch):
    output = iter(["PX0= 1\nTICKS= 10\nDONE", "PX0= 1\nTICKS= 11\nDONE"])
    commands = []
    def launch(work, mount, lines, **kwargs):
        commands.extend(lines)
        return SimpleNamespace(finished=True, timed_out=False)
    monkeypatch.setattr(bench, "launch", launch)
    monkeypatch.setattr(bench, "read_dos", lambda *a: next(output))
    assert bench.run("v-g3", Path("OPT.EXE"), 100, 2, expected=("PX0= 1", "DONE")) == [10, 11]
    assert commands == ["OPT.EXE 100 > OPT0.TXT", "OPT.EXE 100 > OPT1.TXT"]
    assert bench.output_name(Path("BASE.EXE"), 0) != bench.output_name(Path("OPT.EXE"), 0)
