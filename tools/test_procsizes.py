import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import procsizes  # noqa: E402


def pubdef(segment: int, entries: list[tuple[str, int]]) -> bytes:
    body = bytes([0, segment]) + b"".join(bytes([len(name)]) + name.encode() + struct.pack("<H", offset) + b"\0" for name, offset in entries)
    return bytes([0x90]) + struct.pack("<H", len(body) + 1) + body + b"\0"


class ProcSizeTests(unittest.TestCase):
    def test_a_procedure_is_the_distance_to_the_next_public_in_its_segment(self):
        """A program's total hid which procedure grew: deedlines COPPER +398 B sat in a +2232 B sum."""
        obj = pubdef(1, [("B", 40), ("A", 0)]) + pubdef(2, [("C", 0), ("D", 7)])
        self.assertEqual(procsizes.sizes(obj), {"A": 40, "C": 7})

    def test_growth_is_listed_largest_first_over_procedures_in_both(self):
        old, new = {"A": 10, "B": 20, "C": 5}, {"A": 12, "B": 50, "D": 9}
        self.assertEqual(procsizes.growth(old, new), [("B", 20, 50), ("A", 10, 12)])


if __name__ == "__main__":
    unittest.main()
