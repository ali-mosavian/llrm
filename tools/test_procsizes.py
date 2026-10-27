import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import procsizes  # noqa: E402


def segdef(length: int) -> bytes:
    body = bytes([0x60]) + struct.pack("<H", length) + bytes([1, 1, 1])
    return bytes([0x98]) + struct.pack("<H", len(body) + 1) + body + b"\0"


def pubdef(segment: int, entries: list[tuple[str, int]]) -> bytes:
    body = bytes([0, segment]) + b"".join(bytes([len(name)]) + name.encode() + struct.pack("<H", offset) + b"\0" for name, offset in entries)
    return bytes([0x90]) + struct.pack("<H", len(body) + 1) + body + b"\0"


class ProcSizeTests(unittest.TestCase):
    def test_a_procedure_is_the_distance_to_the_next_public_in_its_segment(self):
        """A program's total hid which procedure grew: deedlines COPPER +398 B sat in a +2232 B sum."""
        obj = pubdef(1, [("B", 40), ("A", 0)]) + pubdef(2, [("C", 0), ("D", 7)])
        self.assertEqual(procsizes.sizes(obj), {"A": 40, "C": 7})

    def test_the_last_public_of_a_segment_runs_to_the_end_of_the_segment(self):
        """A Nib object has one public, so its only function had no size and a +162 B growth read as nothing."""
        obj = segdef(1273) + pubdef(1, [("_main", 0)]) + segdef(64) + pubdef(2, [("A", 4), ("B", 20)])
        self.assertEqual(procsizes.sizes(obj), {"_main": 1273, "A": 16, "B": 44})

    def test_growth_is_listed_largest_first_over_procedures_in_both(self):
        old, new = {"A": 10, "B": 20, "C": 5}, {"A": 12, "B": 50, "D": 9}
        self.assertEqual(procsizes.growth(old, new), [("B", 20, 50), ("A", 10, 12)])


if __name__ == "__main__":
    unittest.main()
