#!/usr/bin/env python3
"""folded_check.py: the rule, on made-up tables. usage: python3 -I test_folded_check.py"""
import os, sys, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ns = {"__name__": "folded_check"}
exec(open(os.path.join(HERE, "folded_check.py")).read(), ns)
folded = ns["folded"]
exempt = ns["exempt"]


class Rule(unittest.TestCase):
    def test_a_program_a_few_bytes_above_the_floor_at_every_level_is_folded(self):
        table = {"__empty__": {"Os": 12, "O2": 12}, "tiny": {"Os": 31, "O2": 31}, "real": {"Os": 300, "O2": 280}}
        self.assertEqual(sorted(folded(table)), ["tiny"])

    def test_o2_under_a_third_of_os_above_the_floor_is_folded(self):
        # float_in_place_update on the z80: 9421 B at -Os, 13 B at -O2
        table = {"__empty__": {"Os": 12, "O2": 12}, "f": {"Os": 9421, "O2": 13}, "ok": {"Os": 400, "O2": 150}}
        self.assertEqual(sorted(folded(table)), ["f"])

    def test_the_floor_is_the_targets_own(self):
        # the same 83 B object is folded where the floor is 70 B and real where it is 12 B
        near = {"__empty__": {"Os": 70, "O2": 70}, "p": {"Os": 83, "O2": 83}}
        far = {"__empty__": {"Os": 12, "O2": 12}, "p": {"Os": 83, "O2": 83}}
        self.assertEqual(sorted(folded(near)), ["p"])
        self.assertEqual(sorted(folded(far)), [])

    def test_a_level_without_a_floor_row_is_refused(self):
        # a missing floor must not read as 0 (or be skipped): -Oz has programs and no `__empty__`
        table = {"__empty__": {"Os": 12, "O2": 12}, "p": {"Os": 100, "Oz": 90, "O2": 80}}
        with self.assertRaises(SystemExit):
            folded(table)

    def test_an_exemption_names_its_reason(self):
        import tempfile
        with tempfile.NamedTemporaryFile("w", suffix=".allow", delete=False) as f:
            f.write("# comment\nos_exit\tan OS shim: the program is its call sequence\n")
        self.assertEqual(exempt(f.name), {"os_exit"})
        with tempfile.NamedTemporaryFile("w", suffix=".allow", delete=False) as g:
            g.write("os_exit\n")
        with self.assertRaises(SystemExit):
            exempt(g.name)

    def test_a_table_without_a_floor_is_refused(self):
        with self.assertRaises(SystemExit):
            folded({"p": {"Os": 100, "O2": 100}})


unittest.main()
