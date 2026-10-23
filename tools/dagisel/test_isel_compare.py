"""isel_compare.py reads the `ISEL_DUMP=1` listing."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import isel_compare as tool  # noqa: E402

DUMP = """=== ISel old
  _f@3: 4 instructions
    block 0x0001
      0x0001     ; delivers v1@AX,v2@DX
      0x0002  v3 := add v2, v1  ; inserted
      0x0003  AX := mov v3  ; inserted
      0x0004  cmp v3, 0x0  ; inserted
      0x0004  jne   ; inserted
    block 0x0005
      v9 := phi 0x0001:v3
      0x0006  ret   ; inserted

=== FarIndirectCalls
  _f@3: 4 instructions
      0x0002  v3 := add v2, v1  ; later stage, not read
"""


def test_the_first_listing_is_read_as_mnemonics_without_copies_or_phis():
    got = tool.listings(DUMP)
    assert dict(got["_f@3"]) == {"add": 1, "cmp": 1, "jne": 1, "ret": 1}
