"""Benchmark failures must not turn into flattering speed measurements."""

import re
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import bench


def test_every_timed_basic_benchmark_uses_the_rdtsc_instrument() -> None:
    """bench.py required TSC stamps while nbody and fpbench still printed PIT ticks."""
    root = Path(__file__).resolve().parents[1] / "bench"
    for name in ("nbody", "fpbench", "nbodys"):
        source = (root / f"{name}.bas").read_text()
        folded = source.casefold()
        assert re.search(
            r"declare\s+sub\s+tscsnap\s*\(\s*hi\s+as\s+long\s*,\s*lo\s+as\s+long\s*\)",
            folded,
        ), name
        assert len(re.findall(r"\bcall\s+tscsnap\s*\(", folded)) == 2, name
        assert re.search(r'\bprint\s+"tsc0="', folded), name
        assert re.search(r'\bprint\s+"tsc1="', folded), name
        assert "pitsnap" not in folded and 'print "ticks="' not in folded, name


@pytest.mark.parametrize(
    "compile_log,link_log,finished,accepted",
    [
        ("1 Severe Error(s)", "Microsoft (R) Segmented Executable Linker\n" * 2, True, False),
        ("", "Microsoft (R) Segmented Executable Linker\n" * 2, True, False),
        (
            "0 Severe Error(s)",
            "Microsoft (R) Segmented Executable Linker\nerror L2029: unresolved external",
            True,
            False,
        ),
        ("0 Severe Error(s)", "", True, False),
        ("0 Severe Error(s)", "Microsoft (R) Segmented Executable Linker\n" * 2, False, False),
        ("    0 Severe  Error(s)\r\n", "Microsoft (R) Segmented Executable Linker\n" * 2, True, True),
    ],
)
def test_failed_build_artifacts_are_not_benchmarked(monkeypatch, tmp_path, compile_log, link_log, finished, accepted):
    """BC can emit an OBJ after severe errors; LINK can emit an EXE with unresolved calls."""
    monkeypatch.setattr(bench, "BUILD", tmp_path)
    monkeypatch.setattr(
        bench,
        "CONFIGS",
        {"test": SimpleNamespace(available=True, mount=tmp_path, bc="BC", link="LINK", runtime="RUNTIME")},
    )
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


@pytest.mark.parametrize(
    "output",
    [
        "PX0= 1\nTSC0= 0 10\nTSC1= 0 10\nDONE",
        "PX0= 1\nTSC0= 0 10\nTSC1= 0 20",
    ],
)
def test_invalid_timer_or_incomplete_answer_is_not_timed(monkeypatch, output):
    """A missing or non-increasing RDTSC interval must not become a timing."""
    monkeypatch.setattr(bench, "launch", lambda *a, **kw: SimpleNamespace(finished=True, timed_out=False))
    monkeypatch.setattr(bench, "read_dos", lambda *a: output)
    with pytest.raises(SystemExit):
        bench.run("v-g3", Path("OPT.EXE"), 100, 1)


def test_timeout_does_not_reuse_an_old_reading(monkeypatch):
    """A leftover valid OUT file cannot make a timed-out run successful."""
    monkeypatch.setattr(bench, "launch", lambda *a, **kw: SimpleNamespace(finished=False, timed_out=True))
    monkeypatch.setattr(
        bench,
        "read_dos",
        lambda *a: "PX0= 1\nTSC0= 0 10\nTSC1= 0 20010\nDONE",
    )
    with pytest.raises(SystemExit):
        bench.run("v-g3", Path("OPT.EXE"), 100, 1)


def test_refusal_is_not_benchmarked_as_optimization(monkeypatch):
    """NBODY's unchanged fallback was previously eligible for an 'optimized' timing."""
    monkeypatch.setattr(
        bench.wholeseg,
        "emitted",
        lambda *a, **kw: SimpleNamespace(
            outcome=bench.wholeseg.Emission.REFUSED, reason="missing byte move", data=b"original"
        ),
    )
    with pytest.raises(SystemExit, match="missing byte move"):
        bench.optimized(b"original", False, "386")


def test_benchmark_defaults_to_the_only_supported_native_fpu_path(monkeypatch):
    """The performance suite stopped before timing when its stale default
    requested the retired software-FPU rewrite path.

    Native x87 is now the production path, so an ordinary benchmark run must
    request it without relying on every caller to remember a compatibility
    flag.
    """
    observed = []

    monkeypatch.setattr(
        bench.wholeseg,
        "emitted",
        lambda data, **options: observed.append(options["native_fpu"])
        or SimpleNamespace(outcome=bench.wholeseg.Emission.LIR, reason="rebuilt", data=data),
    )

    assert bench.optimized(b"object") == b"object"
    assert observed == [True]


def test_wrong_answer_is_not_a_speedup(monkeypatch):
    """A faster but wrong floating benchmark previously supplied a quoted speedup."""
    monkeypatch.setattr(bench, "launch", lambda *a, **kw: SimpleNamespace(finished=True, timed_out=False))
    monkeypatch.setattr(
        bench,
        "read_dos",
        lambda *a: "PX0= -2147483648\nTSC0= 0 10\nTSC1= 0 20010\nDONE",
    )
    with pytest.raises(SystemExit, match="answer mismatch"):
        bench.run("v-g3", Path("OPT.EXE"), 100, 1, expected=("PX0= 1", "DONE"))


def test_each_repetition_is_checked_and_both_outputs_are_preserved(monkeypatch):
    output = iter(
        [
            "PX0= 1\nTSC0= 0 100\nTSC1= 0 110\nDONE",
            "PX0= 1\nTSC0= 0 100\nTSC1= 0 111\nDONE",
        ]
    )
    commands = []

    def launch(work, mount, lines, **kwargs):
        commands.extend(lines)
        return SimpleNamespace(finished=True, timed_out=False)

    monkeypatch.setattr(bench, "launch", launch)
    monkeypatch.setattr(bench, "read_dos", lambda *a: next(output))
    monkeypatch.setattr(bench, "_counts_per_ms", lambda: 1)
    assert bench.run("v-g3", Path("OPT.EXE"), 100, 2, expected=("PX0= 1", "DONE")) == [10, 11]
    assert commands == ["OPT.EXE 100 > OPT0.TXT", "OPT.EXE 100 > OPT1.TXT"]
    assert bench.output_name(Path("BASE.EXE"), 0) != bench.output_name(Path("OPT.EXE"), 0)
