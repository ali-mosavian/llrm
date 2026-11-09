"""tools/arm64/count.py never picks the counted range itself."""

import os
import subprocess
import sys
import unittest

COUNT = os.path.join(os.path.dirname(__file__), "arm64", "count.py")


class CountRange(unittest.TestCase):
    def test_no_range_flag_is_an_error_not_a_whole_program_count(self):
        # Without a flag the count ran over the whole program and was read as the kernel's.
        done = subprocess.run([sys.executable, COUNT, "x.elf"], capture_output=True, text=True)
        self.assertEqual(done.returncode, 2, done.stdout + done.stderr)
        self.assertIn("--kernel", done.stderr)
        self.assertIn("--whole", done.stderr)

    def test_both_flags_are_an_error(self):
        done = subprocess.run([sys.executable, COUNT, "--kernel", "--whole", "x.elf"], capture_output=True, text=True)
        self.assertEqual(done.returncode, 2, done.stderr)


class KernelRange(unittest.TestCase):
    def test_a_helper_the_linker_places_after_main_is_in_the_kernel(self):
        # jpeg_single: `getbit` linked after `main`; the range ended at main and its 965,879 executed
        # instructions (ours) / 1,881,194 (clang no-vec) were in no kernel count.
        import importlib.util

        spec = importlib.util.spec_from_file_location("count", COUNT)
        count = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(count)
        funcs = {"bench": 0x100, "emit": 0x140, "main": 0x200, "getbit": 0x220, "report": 0x300, "memcpy": 0x320}
        lo, hi, skip = count.kernel_range(funcs, {"main": 0x1C})
        inside = lambda a: lo <= a < hi and not skip[0] <= a < skip[1]
        self.assertTrue(inside(0x100) and inside(0x140), "the kernels before main")
        self.assertTrue(inside(0x230), "getbit, after main")
        self.assertFalse(inside(0x204), "main itself")
        self.assertFalse(inside(0x300) or inside(0x330), "the runtime")


if __name__ == "__main__":
    unittest.main()
