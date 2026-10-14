"""`uv run --project tools python -m unittest discover -s tools/loops`"""

import sys
import unittest
from argparse import Namespace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import run  # noqa: E402


class ReferenceTests(unittest.TestCase):
    def test_a_run_without_references_may_not_write_the_known_shortfalls(self):
        """A --no-refs run judged every case "ideal" and reported 26,880 new
        shortfalls against a file written with references."""
        langs = ["c", "bas", "nib"]
        self.assertFalse(run.lacks_references(Namespace(no_refs=False, inline=False), langs))
        self.assertTrue(run.lacks_references(Namespace(no_refs=True, inline=False), langs))
        self.assertTrue(run.lacks_references(Namespace(no_refs=False, inline=True), langs))
        self.assertTrue(run.lacks_references(Namespace(no_refs=False, inline=False), ["bas"]))

    def test_a_loop_is_not_judged_ideal_where_no_reference_was_built(self):
        """Without reference facts every bound read as one no compiler met, so
        every ivs shortfall was filed as ivs-ideal."""
        result = run.Result()
        self.assertIsNone(run.ivs_kind(result, "case", "486-O2", 1, references=False))
        self.assertEqual(run.ivs_kind(result, "case", "486-O2", 1, references=True), "ivs-ideal")


class DumpPlaceTests(unittest.TestCase):
    MOUNTS = "/dev/nvme0n1p4 / ext4 rw 0 0\ntmpfs /tmp tmpfs rw 0 0\n/dev/sdb1 /tmp/disk ext4 rw 0 0\n"

    def test_a_dump_under_a_tmpfs_is_refused(self):
        """A run's dump filled /tmp's inodes and stopped six sessions."""
        self.assertTrue(run.on_tmpfs(Path("/tmp/lp"), self.MOUNTS))
        self.assertTrue(run.on_tmpfs(Path("/tmp"), self.MOUNTS))

    def test_a_dump_elsewhere_is_not(self):
        self.assertFalse(run.on_tmpfs(Path("/home/alim/loops"), self.MOUNTS))
        self.assertFalse(run.on_tmpfs(Path("/tmp/disk/loops"), self.MOUNTS))
        self.assertFalse(run.on_tmpfs(Path("/tmpfoo/loops"), self.MOUNTS))


def loop(size, memory, invariant=0, calls=0):
    return Namespace(size=size, memory=memory, invariant_loads=invariant, calls=calls)


class InvariantLoadsTests(unittest.TestCase):
    """A reload of what the loop never changes was a shortfall whatever the references did: 150 of the 190 x_ kernels
    that had one were already no larger than both references (x_usemod: 11 instructions and 3 memory operands against
    gcc's 13 and 4 and Open Watcom's 14 and 4)."""

    def test_a_reload_is_no_shortfall_where_llrm_is_no_larger_than_every_reference(self):
        self.assertEqual(run.invariant_loads([loop(11, 3, invariant=2)], [(14, 4), (13, 4)]), ("invariant-loads", False))

    def test_a_reload_is_a_shortfall_where_a_reference_has_fewer_memory_operands_or_fewer_instructions(self):
        self.assertEqual(run.invariant_loads([loop(9, 5, invariant=2)], [(12, 4), (12, 6)]), ("invariant-loads", True))
        self.assertEqual(run.invariant_loads([loop(8, 3, invariant=1)], [(7, 4)]), ("invariant-loads", True))

    def test_a_loop_with_no_reload_falls_short_of_nothing(self):
        self.assertEqual(run.invariant_loads([loop(20, 9)], [(5, 1)]), ("invariant-loads", False))

    def test_a_run_without_references_judges_nothing(self):
        self.assertIsNone(run.invariant_loads([loop(11, 3, invariant=2)], [], references=False))

    def test_references_that_gave_no_bar_leave_the_reload_an_ideal_not_dropped(self):
        """x_elemf64_usesquare: Open Watcom's loop calls a helper and gcc built none. Its loop reloads two x87 control
        words every trip: #1638 judged it not short, and the gap went out of view."""
        self.assertEqual(run.invariant_loads([loop(16, 6, invariant=3)], []), ("invariant-loads-ideal", True))
        self.assertEqual(run.invariant_loads([loop(9, 2)], []), ("invariant-loads-ideal", False))

    def test_a_reference_that_calls_a_helper_is_no_bar(self):
        result = run.Result()
        result.facts[("case", "ow", "486-O2")] = [loop(2, 0, calls=1)]
        result.facts[("case", "gcc", "486-O2")] = [loop(10, 3)]
        self.assertEqual(run.reference_bars(result, "case", "486-O2"), [(10, 3)])


if __name__ == "__main__":
    unittest.main()
