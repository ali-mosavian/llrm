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


class Join(unittest.TestCase):
    def test_ours_underscored_names_meet_the_pins(self):
        """Ours printed `_add_bits`, the pin `add_bits`: nothing joined, and the run printed `DELTAS {}` and `TOTAL 0 0 functions 0` as if the two agreed."""
        mine = slotcmp.drop_symbol_prefix({"_add_bits": 120, "___mulsf3": 112})
        self.assertEqual(slotcmp.common(mine, {"add_bits": 119, "__mulsf3": 117}), ["__mulsf3", "add_bits"])

    def test_an_empty_join_is_refused(self):
        with self.assertRaises(SystemExit):
            slotcmp.common({"_add_bits": 120}, {"add_bits": 119})


if __name__ == "__main__":
    unittest.main()
