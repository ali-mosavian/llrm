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
    assert copywhy.our_counts(DUMP) == {("Coalescer", "f@3"): 1}
