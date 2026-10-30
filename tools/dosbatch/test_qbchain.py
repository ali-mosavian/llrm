"""CHAIN runs the next program, built by our compiler as under BC: two programs print the same text.

A stand-alone build (BCOM45) does not carry COMMON values to the chained program; BC's prints 0 and an empty string
for them, and so does ours. Under the run-time module BRUN45 the values travel (`--run-time-module` builds for it).

`uv run --project tools python -m unittest discover -s tools/dosbatch -p test_qbchain.py`"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402

SOURCES = Path(__file__).resolve().parents[2] / "tests" / "chain" / "qb"
NAMES = ("chfirst", "chsecond")
WANT = "first\nsecond 0  0 \n"
WANT_RTM = "first\nsecond 7  123456 carried\n"
BRUN = dosbatch.Toolchain(dosbatch.QB45, r"V:\BC", r"V:\LINK", r"V:\LIB\BRUN45.LIB")
BRUN_EXE = dosbatch.QB45 / "BRUN45.EXE"


def ours(work: Path) -> str:
    work.mkdir(parents=True, exist_ok=True)
    jobs = []
    for name in NAMES:
        obj = work / f"{name}.obj"
        argv = [str(dosbatch.BIN / "llrm-qb"), str(SOURCES / f"{name}.bas"), "--dialect", "qb45", "--runtime", "qb45", "-O2", "-march=i486", "-o", str(obj)]
        done = subprocess.run(dosbatch.with_isel(argv), capture_output=True, text=True)
        assert done.returncode == 0, done.stderr
        jobs.append(dosbatch.Job(name, "obj", obj))
    return dosbatch.run(jobs, work / "run")["chfirst"].text.replace("\r\n", "\n")


def bc(work: Path) -> str:
    jobs = [dosbatch.Job(name, "bas", SOURCES / f"{name}.bas", switches="/O") for name in NAMES]
    return dosbatch.run(jobs, work / "run")["chfirst"].text.replace("\r\n", "\n")


def run_time_module(work: Path, built_by_bc: bool) -> str:
    """The pair linked against BRUN45.LIB and run with BRUN45.EXE beside them."""
    work.mkdir(parents=True, exist_ok=True)
    jobs = []
    for name in NAMES:
        if built_by_bc:
            jobs.append(dosbatch.Job(name, "bas", SOURCES / f"{name}.bas", switches="", files=(BRUN_EXE,)))
            continue
        obj = work / f"{name}.obj"
        argv = [str(dosbatch.BIN / "llrm-qb"), str(SOURCES / f"{name}.bas"), "--dialect", "qb45", "--runtime", "qb45", "--run-time-module", "-O2", "-march=i486", "-o", str(obj)]
        done = subprocess.run(dosbatch.with_isel(argv), capture_output=True, text=True)
        assert done.returncode == 0, done.stderr
        jobs.append(dosbatch.Job(name, "obj", obj, files=(BRUN_EXE,)))
    return dosbatch.run(jobs, work / "run", tools=BRUN)["chfirst"].text.replace("\r\n", "\n")


class ChainTests(unittest.TestCase):
    def test_the_chained_program_runs_after_the_first_as_it_does_under_bc(self):
        """CHAIN was refused; its program's text, first then the chained one's, is BC's."""
        with tempfile.TemporaryDirectory() as directory:
            oracle = bc(Path(directory) / "bc")
            mine = ours(Path(directory) / "ours")
        self.assertEqual(oracle, WANT)
        self.assertEqual(mine, oracle)

    def test_under_the_run_time_module_the_common_values_travel_as_under_bc(self):
        """Linked with BRUN45.LIB our object crashed at start (LINK: no stack segment); it now runs, and CHAIN carries COMMON."""
        with tempfile.TemporaryDirectory() as directory:
            oracle = run_time_module(Path(directory) / "bc", True)
            mine = run_time_module(Path(directory) / "ours", False)
        self.assertEqual(oracle, WANT_RTM)
        self.assertEqual(mine, oracle)


if __name__ == "__main__":
    unittest.main()
