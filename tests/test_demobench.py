"""demobench's reading of a mark."""

import sys
import struct
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))


def test_a_mark_is_the_time_stamp_difference_across_the_32_bit_boundary(tmp_path) -> None:
    """The PIT reader was off by whole BIOS ticks; RDTSC passes 2^32 in under a minute of a demo."""
    import demobench

    before = (1 << 32) - 75_000
    after = before + 3 * 75_000

    def signed(word):
        return struct.unpack("<i", struct.pack("<I", word))[0]

    stamps = (before >> 32, before & 0xFFFFFFFF, after >> 32, after & 0xFFFFFFFF, 7, 9)
    header = b"\xfd" + bytes(6)
    (tmp_path / "T1.BIN").write_bytes(header + struct.pack("<6i", *map(signed, stamps)))
    elapsed, checksum, _dump = demobench.marks(tmp_path)[1]
    assert elapsed == 3.0
    assert checksum == (7, 9)
