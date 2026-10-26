"""tools/copywhy.py counts a register copy, not a store of a register."""
import copywhy

DUMP = """=== Coalescer
  f@3: 4 instructions
    block 0x0000
      0x0001  v2 := mov v1  ; inserted
      0x0002  [[bp-0x12a]] := mov v46  ; inserted
      0x0003  v3 := mov [None v1@unplaced]  ; inserted
      0x0004  v4 := mov 0x0  ; inserted
"""


def test_a_store_of_a_register_is_no_copy():
    """A store `[bp-N] := mov vN` was counted as a copy: nightmare_selftest read 68 left copies, the real count was 13."""
    assert copywhy.our_counts(DUMP) == {("Coalescer", "f"): 1}


def test_a_function_header_without_an_id_is_read():
    """The dump names a function `_f: N instructions`, not `f@3:`: every count was None and x_switch's copies read as no data."""
    assert copywhy.our_counts(DUMP.replace("f@3", "_f")) == {("Coalescer", "_f"): 1}


def test_the_pin_runs_as_m32_when_a_triple_is_named():
    """The pin's target was fixed at code16: x_switch's m32 counts compared our i486 code with its 16-bit code."""
    names, target = copywhy.split_target(["f", "--triple", "i386-unknown-linux-gnu", "--cpu", "i486", "g"])
    assert (names, target) == (["f", "g"], ["-mtriple=i386-unknown-linux-gnu", "-mcpu=i486"])
    assert copywhy.split_target(["f"]) == (["f"], copywhy.CODE16)
