"""tools/dosbatch/qbrt_run.py: the selection and the compile report."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qbrt_run  # noqa: E402


def programs(tmp_path: Path) -> dict[str, Path]:
    for name, text in {"plain": "PRINT 1\n", "handler": "ON ERROR GOTO h\nPRINT 2\nh: RESUME NEXT\n", "erl": "PRINT ERL\n"}.items():
        (tmp_path / f"{name}.bas").write_text(text)
    return {name: tmp_path / f"{name}.bas" for name in ("plain", "handler", "erl")}


def test_the_default_selects_the_error_handling_programs_all_selects_every_program_and_only_the_named(tmp_path):
    """The on-error subset was the only selector: a run of 'the qbrt group' covered 14 of tests/qbrt's programs and str_slices was never in it."""
    sources = programs(tmp_path)
    assert qbrt_run.select(sources) == ["erl", "handler"]
    assert qbrt_run.select(sources, everything=True) == ["erl", "handler", "plain"]
    assert qbrt_run.select(sources, everything=True, only=("plain",)) == ["plain"]


def test_a_program_the_oracle_route_does_not_compile_is_named_not_skipped(tmp_path):
    sources = programs(tmp_path)
    kept, failed = qbrt_run.compiling(["plain", "handler"], sources, lambda source, obj: "B$SCMP" if source.stem == "plain" else None, tmp_path)
    assert (kept, failed) == (["handler"], {"plain": "B$SCMP"})


def test_all_lists_every_tests_qbrt_program_the_differential_compares():
    """Every tests/qbrt probe but the pictures (gfx_), the flat ports and the programs with no BCOM45 run to differ from (test_qbrt.LLRM_ONLY)."""
    import dosbatch
    import test_qbrt

    on_disk = {
        path.stem
        for path in (dosbatch.ROOT / "tests" / "qbrt").glob("*.bas")
        if not path.name.startswith("gfx_") and ".flat" not in path.name and not path.stem.startswith(test_qbrt.LLRM_ONLY)
    }
    everything = set(qbrt_run.select(test_qbrt.SOURCES, everything=True))
    assert on_disk <= everything and len(on_disk) > 50
    assert len(qbrt_run.select(test_qbrt.SOURCES)) < len(on_disk), "the default is the error-handling subset"


def test_a_level_is_taken_with_the_dash_without_it_and_after_an_equals_sign():
    """`--level -O1` was refused by argparse (the dash reads as an option of its own): oversight lost runs to it; `--level=-O1` worked."""
    for spelling in (["--level", "-O1"], ["--level=-O1"], ["--level", "O1"], ["--level", "1"]):
        assert qbrt_run.parse(["w", *spelling]).level == "-O1", spelling
    assert qbrt_run.parse(["w", "--level", "-Os", "--all"]).all and qbrt_run.parse(["w", "--level", "s"]).level == "-Os"
    assert qbrt_run.parse(["w"]).level == "-O2"


def test_the_tool_loads_without_pytest():
    """test_qbrt imports pytest, so `python3 qbrt_run.py` outside uv died with ModuleNotFoundError: oversight lost runs to it."""
    import subprocess

    done = subprocess.run(
        [sys.executable, "-c", "import sys; sys.modules['pytest'] = None; sys.path.insert(0, sys.argv[1]); import qbrt_run; qbrt_run.load()", str(Path(__file__).parent)],
        capture_output=True,
        text=True,
    )
    assert done.returncode == 0, done.stderr[-300:]


def run_of(difference: str = "", status: str = "ok"):
    from types import SimpleNamespace as N

    return N(reference=N(status="ok"), candidate=N(status=status, detail=None), difference=difference, screen_difference="")


def test_a_known_difference_is_expected_and_one_that_now_matches_fails():
    """qbrt_run counted `scores` at the flat target as a difference although test_qbrt.KNOWN lists it: a clean run read 217/218 and hid new failures."""
    known = {("scores", "flat"): "#1542"}
    assert qbrt_run.unexpected(known, "scores", "flat", run_of("x")) is None
    assert "remove it from KNOWN" in qbrt_run.unexpected(known, "scores", "flat", run_of())
    assert qbrt_run.unexpected(known, "other", "flat", run_of("x")) == "x"
    assert qbrt_run.unexpected(known, "other", "flat", run_of()) is None


def test_a_reference_and_candidate_that_agree_but_are_not_real_bcs_text_are_named():
    """real_print_wide: 9 lines unlike real BC (' 10000000' for ' 9999999.999999994') with reference == candidate, so the byte-for-byte differential passed."""
    import qbrt_programs as q

    stored = [" 9999999.999999994"]
    wrong = " 10000000\r\n"
    assert "agree but are not real BC's text" in q.not_real_bc(stored, wrong, wrong)
    assert "candidate is not real BC's text" in q.not_real_bc(stored, " 9999999.999999994\n", wrong)
    assert q.not_real_bc(stored, " 9999999.999999994 \r\n", " 9999999.999999994\n") is None
