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
