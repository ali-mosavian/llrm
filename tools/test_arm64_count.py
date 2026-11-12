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


    def load(self):
        import importlib.util

        spec = importlib.util.spec_from_file_location("count", COUNT)
        count = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(count)
        return count

    LAYOUT = (
        {"bench": 0x100, "emit": 0x140, "main": 0x200, "getbit": 0x220, "report": 0x300, "memcpy": 0x320},
        {"bench": 0x40, "emit": 0xC0, "main": 0x1C, "getbit": 0x80, "report": 0x20, "memcpy": 0x40},
    )

    def test_every_address_of_the_layout_is_in_one_class(self):
        count = self.load()
        ranges = count.classes(*self.LAYOUT)
        owner = lambda a: count.class_of(ranges, a)
        self.assertEqual([owner(a) for a in (0x100, 0x140, 0x204, 0x230, 0x300, 0x330, 0x400)],
                         ["kernel", "kernel", "main", "kernel", "runtime", "runtime", "other"])

    def test_a_range_that_loses_instructions_fails_the_sum_guard(self):
        # The old range [first, main) put getbit's instructions in no class: 5 of 105 lost.
        count = self.load()
        count.check_sum({"kernel": 70, "main": 10, "runtime": 20, "other": 0}, 100)
        with self.assertRaises(SystemExit):
            count.check_sum({"kernel": 70, "main": 10, "runtime": 20}, 105)
        with self.assertRaises(SystemExit):
            count.check_sum({"kernel": 70, "main": 10, "runtime": 20, "other": 5}, 105)


    def test_a_signal_file_is_generated_for_the_stft_programs(self):
        # find_input found no SIGNAL (generated, not stored): stft read -1 and every column "agreed" on a
        # 3,286 instruction run in place of 21,900,690.
        import hashlib

        count = self.load()
        path = count.find_input("signal")
        self.assertEqual(len(open(path, "rb").read()), 131072)
        self.assertEqual(hashlib.md5(open(path, "rb").read()).hexdigest(), "a7e4387b9cd82c5726d09f56a2b21933")

    def test_a_failed_program_is_not_the_expected_value(self):
        sys.path.insert(0, os.path.join(os.path.dirname(COUNT)))
        import expect

        self.assertTrue(expect.same_value("30825279", "30825279\n"))
        self.assertTrue(expect.same_value("2890111581:89e49c23", "-1404855715\n"))
        self.assertFalse(expect.same_value("-1", "30825279\n"))
        self.assertEqual(expect.status("stft_fixed", "O2", "-1", "30825279\n"), "fail")

    def test_the_stft_single_value_is_a_named_exception_only_at_its_observed_value(self):
        sys.path.insert(0, os.path.join(os.path.dirname(COUNT)))
        import expect

        for level in ("O1", "O2", "O3", "Os"):
            self.assertEqual(expect.status("stft_single", level, "40144774:f14311fc", "78803551\n"), "exception", level)
        self.assertEqual(expect.status("stft_single", "O2", "-1", "78803551\n"), "fail")
        self.assertEqual(expect.status("stft_single", "O2", "502187692", "78803551\n"), "fail")
        self.assertEqual(expect.status("stft_single", "O2", "78803551", "78803551\n"), "ok")

    def test_the_level_may_be_given_as_cell_sh_passes_it(self):
        # cell.sh passes 1/2/3/s, the table is keyed O1/O2/O3/Os: every stft_single cell of the
        # judge table came out `unmeasured:value 40144774...` instead of `exception`.
        sys.path.insert(0, os.path.join(os.path.dirname(COUNT)))
        import expect

        for level in ("1", "2", "3", "s", "z"):
            self.assertEqual(expect.status("stft_single", level, "40144774:f14311fc", "78803551\n"), "exception", level)


    def test_a_program_with_two_reports_is_judged_on_its_last(self):
        # grep prints lines then a checksum; `reported` holds the checksum, the .out's first number (4745 lines) never equalled it:
        # all 188 grep cells of the LV-X2 table read `unmeasured:value ... is not the expected one`.
        import expect

        ci_out = "115\n494962839\n"
        self.assertEqual(expect.status("grep", "O2", "494962839", ci_out), "ok")
        self.assertEqual(expect.status("grep", "O2", "115", ci_out), "fail")
        self.assertEqual(expect.status("grep", "O2", "109617741", ci_out), "fail")  # rt.c's old 64 KiB of fixed text

    def test_the_dickens_corpus_is_found_by_its_dos_name(self):
        sys.path.insert(0, os.path.join(os.path.dirname(COUNT), "..", "dosbatch"))
        import corpus

        count = self.load()
        try:
            want = str(corpus.path("dickens"))
        except corpus.Unavailable as why:
            self.skipTest(str(why))
        self.assertEqual(count.find_input("dickens"), want)


if __name__ == "__main__":
    unittest.main()
