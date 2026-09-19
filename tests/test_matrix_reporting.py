"""
The gate has to say what kind of failure it saw.

A wrong answer and no answer look identical in a count and mean opposite
things: the first is this pass miscompiling, the second is the harness or
the machine. One matrix run came back "11 of 12" with nothing in that line
to say which, and three runs since have been clean -- so the difference is
now in the output, and these tests are what keep it there.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import e2e
import matrix
from dosbox import Run


def test_a_wrong_answer_and_no_answer_are_different_kinds() -> None:
    """The classification the summary rests on."""
    assert "DIFF" in matrix.MISCOMPILE
    assert "BASEDIFF" in matrix.MISCOMPILE
    for status in ("TIMEOUT", "NODONE", "BCFAIL", "LINKFAIL", "RUNFAIL", "REWRITEFAIL"):
        assert status not in matrix.MISCOMPILE, f"{status} is not the pass computing the wrong thing"


def test_pass_is_not_a_failure_of_either_kind() -> None:
    assert "PASS" not in matrix.MISCOMPILE
    assert e2e.Verdict("x", "PASS", "").ok
    assert not e2e.Verdict("x", "TIMEOUT", "").ok


def test_a_killed_run_is_reported_as_a_timeout_not_as_stopping_early(tmp_path: Path) -> None:
    """judge() gets the Run so it can tell the two apart.

    link_and_run used to discard it, which is what made a loaded machine
    indistinguishable from a program the rewrite had hung.
    """
    golden = tmp_path / "golden"
    golden.mkdir()
    (golden / "prog.txt").write_text("A=1\nDONE\n")
    work = tmp_path / "work"
    work.mkdir()
    (work / "PROG.OBJ").write_bytes(b"\0")
    # both runs cut short, exactly as a kill at the deadline leaves them
    (work / "B_PROG.TXT").write_text("A=1\nDONE\n")
    (work / "O_PROG.TXT").write_text("A=1\n")

    killed = e2e.judge(work, "prog", golden, "", Run(finished=False, timed_out=True, seconds=300.0))
    assert killed.status == "TIMEOUT"
    assert "300s" in killed.detail

    on_its_own = e2e.judge(work, "prog", golden, "", Run(finished=True, timed_out=False, seconds=12.0))
    assert on_its_own.status == "NODONE"


def test_a_real_difference_is_still_a_difference_even_after_a_timeout(tmp_path: Path) -> None:
    """The timeout branch must not swallow a genuine miscompile.

    It only fires where the output is short. A complete run that printed the
    wrong thing is DIFF whatever the emulator did.
    """
    golden = tmp_path / "golden"
    golden.mkdir()
    (golden / "prog.txt").write_text("A=1\nDONE\n")
    work = tmp_path / "work"
    work.mkdir()
    (work / "PROG.OBJ").write_bytes(b"\0")
    (work / "B_PROG.TXT").write_text("A=1\nDONE\n")
    (work / "O_PROG.TXT").write_text("A=2\nDONE\n")

    got = e2e.judge(work, "prog", golden, "", Run(finished=False, timed_out=True, seconds=300.0))
    assert got.status == "DIFF"


def test_long_program_names_use_the_same_dos_output_stem_when_judged(tmp_path: Path) -> None:
    """ALGEBRA ran and printed correctly but the gate looked for an impossible 9-character DOS stem."""
    golden = tmp_path / "golden"
    golden.mkdir()
    (golden / "algebra.txt").write_text("RESULT= 702774\nDONE\n")
    work = tmp_path / "work"
    work.mkdir()
    (work / "ALGEBRA.OBJ").write_bytes(b"\0")
    (work / "B_ALGEBR.TXT").write_text("RESULT= 702774\nDONE\n")
    (work / "O_ALGEBR.TXT").write_text("RESULT= 702774\nDONE\n")

    got = e2e.judge(work, "algebra", golden)

    assert got.status == "PASS"
