import os
import tempfile
import unittest

from compact_unwind_diff import compare, entries, main

TABLE = """Unwind info:

Contents of __compact_unwind section:
  Entry at offset 0x0:
    start:                0x0 _f
    length:               0x98
    compact encoding:     0x04000000
"""


class CompactUnwindDiff(unittest.TestCase):
    def test_an_object_without_the_table_is_an_error_not_zero_differences(self):
        """The first run read an object built without -funwind-tables as '0 differences'."""
        with self.assertRaises(ValueError):
            entries("Unwind info:\n")

    def test_a_table_is_read_per_function(self):
        self.assertEqual(entries(TABLE.replace('0x0 _f', '0x98 _f')), {'_f': 0x04000000})

    def test_the_first_function_is_keyed_by_address_not_by_the_section_label(self):
        """clang names it `ltmp0`; `many` was missing from the stress comparison."""
        text = TABLE.replace('_f', 'ltmp0')
        self.assertEqual(entries(text), {'@0': 0x04000000})

    def test_dwarf_fallback_and_mask_differences_are_told_apart(self):
        pin = {'_a': 0x04000000, '_b': 0x04000007, '_c': 0x02000000}
        ours = {'_a': 0x03000000, '_b': 0x04000000, '_c': 0x04000000}
        self.assertEqual(compare(pin, ours), {'_a': 'dwarf', '_b': 'mask', '_c': 'mode'})

    def test_the_command_exits_one_when_an_object_has_no_entries(self):
        with tempfile.TemporaryDirectory() as directory:
            empty = os.path.join(directory, 'objdump')
            with open(empty, 'w') as handle:
                handle.write('#!/bin/sh\necho "Unwind info:"\n')
            os.chmod(empty, 0o755)
            self.assertEqual(main(['a.o', 'b.o', '--objdump', empty]), 1)


if __name__ == '__main__':
    unittest.main()
