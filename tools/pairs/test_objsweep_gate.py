"""The gate's objects leg: a leg passes on the changes it lists and on nothing else."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import objsweep_gate as gate


def line(changed=0, identical=140, both=2, one=0, cells=142):
    return f"objsweep --isel=dag old a new b: {changed} objects changed (m64, O2,Os); {identical} identical, {both} failed on both sides, {one} failed on one side of {cells}"


def test_a_leg_with_no_change_and_no_listing_passes():
    assert gate.verdict("m64", line(), {}) is None


def test_a_change_nobody_listed_fails():
    """A refactor that moved 57 m64 objects passed because the sweep was never run over m64."""
    assert "57 objects changed, 0 expected" in gate.verdict("m64 debug", line(changed=57, identical=83), {})


def test_a_listed_change_passes_and_a_different_count_fails():
    expected = {"m64 debug": {"changed": 57, "why": "entry values"}}
    assert gate.verdict("m64 debug", line(changed=57, identical=83), expected) is None
    assert "58 objects changed, 57 expected" in gate.verdict("m64 debug", line(changed=58, identical=82), expected)


def test_a_leg_that_compiled_little_proves_nothing():
    """Half the compiles failing on both sides is not a pass: they are neither identical nor changed."""
    assert "only 20 of 142" in gate.verdict("m32", line(identical=20, both=122), {})


def test_an_object_compiled_on_one_side_fails():
    assert "one side only" in gate.verdict("m32", line(one=1, identical=139), {})


def test_a_seeded_bad_object_is_caught():
    """The control: one flipped byte must be a different object, and a leg that saw it must fail."""
    assert gate.seeded(b"\x7fELF\x01\x02") is None


def test_the_m16_clang_of_the_pin_reaches_llrm_cc(tmp_path, monkeypatch):
    """Without LLRM_CLANG_M16 every m16 cell failed on both sides: the m16 legs measured nothing."""
    import llrmbin

    sha = (llrmbin.REPO / "toolchain/clang/pin").read_text().splitlines()[1].split()[0]
    clang = tmp_path / "scratch/clang-by-sha" / sha / "bin/clang"
    clang.parent.mkdir(parents=True)
    clang.touch()
    monkeypatch.setattr(Path, "home", lambda: tmp_path)
    monkeypatch.delenv("LLRM_CLANG_M16", raising=False)
    assert llrmbin.cc_environment()["LLRM_CLANG_M16"] == str(clang)
