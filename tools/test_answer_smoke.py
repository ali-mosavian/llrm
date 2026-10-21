"""tools/answer_smoke.py: a wrong answer turns the exit code red."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import answer_smoke  # noqa: E402


def bench(tmp_path: Path, expected: str) -> Path:
    (tmp_path / "p").mkdir()
    (tmp_path / "p" / "p.bas").write_text("PRINT 1\n")
    (tmp_path / "p" / "p.out").write_text(expected)
    return tmp_path


def test_a_wrong_answer_on_either_target_is_counted_and_a_right_one_is_not(tmp_path):
    ran = lambda source, target: ("ok", " 42\r\n")  # noqa: E731
    right = answer_smoke.check(("p",), ran, bench(tmp_path, "42\n"))
    assert [one.split()[0] for one in right] == ["ok", "ok"]
    wrong = answer_smoke.check(("p",), ran, tmp_path)
    assert wrong == right
    (tmp_path / "p" / "p.out").write_text("43\n")
    red = answer_smoke.check(("p",), ran, tmp_path)
    assert [one.split()[0] for one in red] == ["WRONG", "WRONG"], "a deliberately wrong expected output is red"


def test_a_program_that_did_not_build_or_end_is_not_a_right_answer(tmp_path):
    root = bench(tmp_path, "42\n")
    lines = answer_smoke.check(("p",), lambda source, target: ("timeout", ""), root)
    assert not any(one.startswith("ok ") for one in lines)


def test_a_demo_that_differs_or_could_not_be_played_is_not_a_right_answer():
    """oversight's standing check is the Microsoft demos played: #3460 broke NIBBLES and GORILLA at m32 (`DEF SEG = 0` refused) and no answer
    check saw it."""
    import dosbatch

    real, flat = dosbatch.REAL_MODE, answer_smoke.qbbuild.FLAT
    lines = answer_smoke.demo_lines(
        {"GORILLA": {real: [], flat: ["ready"]}, "NIBBLES": {real: [], flat: "VARSEG is 0 and PEEK, POKE take the whole address"}, "X": RuntimeError("never settled")}
    )
    assert [one.split(":")[0].split(" (")[0] for one in lines] == ["ok GORILLA m16", "WRONG GORILLA m32", "ok NIBBLES m16", "NIBBLES m32", "X"]
    assert not any(one.startswith("ok") for one in lines if "m32" in one or one.startswith("X"))


def test_a_string_compare_or_a_float_print_that_is_not_real_bcs_is_red():
    """str_slices on the tip: `@llrm.qb.B$SCMP's flags read other than by a comparison`; real_print_wide / pow_unrolled: `10# ^ i%` printed 10000000
    where real BC prints 9999999.999999994, at every level."""
    green = answer_smoke.strings(lambda: (0, "9 programs: 9 pass, 0 known, 0 fail\n"))
    assert green == [f"ok {name} m16" for name in ("strcmp_smoke", "pow_unrolled")]
    red = answer_smoke.strings(lambda: (1, "FAIL  qb/pow_unrolled.bas [-Os]: line 1: want ' 9999999.999999994', got ' 10000000'\n1 programs: 0 pass, 0 known, 1 fail\n"))
    assert len(red) == 1 and red[0].startswith("WRONG") and "pow_unrolled" in red[0]
    assert answer_smoke.strings(lambda: (2, ""))[0].startswith("WRONG")
    known = answer_smoke.strings(lambda: (0, "KNOWN qb/pow_unrolled.bas [-Os] (#3486): line 1: want 'x'\n4 programs: 0 pass, 4 known, 0 fail\n"))
    assert len(known) == 1 and known[0].startswith("WRONG"), "a program marked known is still a wrong answer in the smoke"
