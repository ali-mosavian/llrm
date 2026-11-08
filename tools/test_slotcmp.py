"""slotcmp compares ours and the pin on one ABI."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import slotcmp  # noqa: E402


class StripRegcall(unittest.TestCase):
    def test_the_ir_both_sides_take_has_no_regcall(self):
        """Counts and sizes across regcall (ours) and sysv (the pin's) were void: rectwo -O2 read +6 B over the pin, -8 B matched."""
        text = "define x86_regcallcc i32 @f(i32 %0) nearcode {\n  %1 = call x86_regcallcc i32 @f(i32 %0)\n}\n"
        self.assertNotIn("regcallcc", slotcmp.strip_regcall(text))


if __name__ == "__main__":
    unittest.main()
