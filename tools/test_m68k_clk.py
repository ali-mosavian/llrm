"""`m68k_clk.parse_clocks` reads the clocks cell of the `cycles` table. A table with no program row (the example found nothing to price) is `None`, not 0: a zero would read as a free program in the O2 judge."""
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_clk import gcc_is_comparable, parse_clocks  # noqa: E402

TABLE = "| exe | insns | 68000 clk | 68000 clk/call |\n608\n| rc.exe | 5035 | 43404 | 14.5 |\n"


class ParseClocks(unittest.TestCase):
    def test_the_clocks_cell_of_the_program_row(self):
        self.assertEqual(parse_clocks(TABLE), "43404")

    def test_a_bound_is_kept_as_a_bound_not_exact(self):
        # `cycles` prints N* for a lower bound; read as exact it made fake gcc cells, dropped it hid pattern_fill 68000 O1 (93048* against gcc 86440)
        self.assertEqual(parse_clocks("| a.exe | 9 | 24634* | 1.0 |\n"), ">=24634")

    def test_uncovered_code_is_unmeasured(self):
        self.assertIsNone(parse_clocks("| a.exe | 9 | 24634? | 1.0 |\n"))

    def test_a_published_maximum_bound_is_kept_as_an_upper_bound(self):
        # memmove_overlap 68020 O2 held two `muls.l` at their maximum: `<=4668` against gcc's 8548 was a blank cell
        self.assertEqual(parse_clocks("| a.exe | 9 | <=4668 | 1.0 |\n"), "<=4668")

    def test_no_program_row_is_none_not_zero(self):
        self.assertIsNone(parse_clocks("| exe | insns | 68000 clk |\n"))
        self.assertIsNone(parse_clocks(""))

    def test_an_empty_clocks_cell_is_none(self):
        self.assertIsNone(parse_clocks("| a.exe | 9 |  | 1.0 |\n"))


if __name__ == "__main__":
    unittest.main()


def test_gcc_pcrel_puts_the_gcc_column_on_the_pc_relative_data_model():
    """The clocks columns compared ours (pc-relative data) with gcc's absolute data: a `lea (sym,%pc)` and an `abs.l` price differently, so a gap mixed the data model with the code."""
    import m68k_clk

    plain = m68k_clk.gcc_command("gcc", "/", "68020", "Os", "p.c", "p.o", False)
    like = m68k_clk.gcc_command("gcc", "/", "68020", "Os", "p.c", "p.o", True)
    assert "-mpcrel" not in plain and "-mpcrel" in like, (plain, like)


def test_gcc_gets_the_programs_no_inline_flag():
    """licm_graph_callee_footprint (`-fno-inline-functions`) priced ours 188244 and gcc 77282 clocks, 2.44x: gcc was built without the flag and inlined `next()` into the loop the test is about."""
    import m68k_clk

    command = m68k_clk.gcc_command("gcc", "/", "68000", "Os", "p.c", "p.o", True, ["-fno-inline-functions"])
    assert "-fno-inline" in command and "-fno-inline-functions" not in command, command


def test_a_pair_that_printed_differently_is_not_priced():
    """u64_divide_wide_divisor 68020 priced gcc at 23 instructions and ours at 137258 for programs whose clocks were compared as if they ran alike; the pair is compared only when stdout and exit match."""
    import m68k_clk

    assert m68k_clk.same_output([(0, "1\n0\n"), (0, "1\n0\n")])
    assert not m68k_clk.same_output([(0, "1\n0\n"), (0, "1\n")])
    assert not m68k_clk.same_output([(0, "x"), None])


def test_cycles_traces_a_68020_program_on_a_68020():
    """The temp exe's path names no CPU, so `cycles` ran every program on qemu's m68000: gcc's 68020 code stopped at its first 68020 instruction and 68 of 87 cells priced a few dozen clocks (loop_copy_directions 4111 against ours 456485906)."""
    import m68k_clk

    assert m68k_clk.cycles_env("68020", "/")["CYCLES_QEMU_CPU"] == "m68020"
    assert m68k_clk.cycles_env("68000", "/")["CYCLES_QEMU_CPU"] == "m68000"


def test_a_program_naming_long_double_has_no_gcc_column():
    """gcc's m68k long double is 12 bytes, ours 8: long_double_layout -Os read ours 2344 B against gcc's 438 B (+435%), a cell no compiler change can move."""
    assert not gcc_is_comparable("long double x; int main(void) { return sizeof x; }")
    assert gcc_is_comparable("// a long double in a comment\nint main(void) { return 0; }")


def test_gcc_builds_the_clang_branch_of_a_program_that_tests_it(tmp_path):
    """variadic.c -Os 68020: gcc built the `#ifndef __clang__` macro walk, 168 B, where ours builds the stdarg branch, 230 B: the card read ours +37% worse; with the same branch gcc is 266 B."""
    import m68k_clk

    source = tmp_path / "p.c"
    source.write_text("#ifdef __clang__\nint a;\n#endif\n")
    assert "-D__clang__" in m68k_clk.gcc_command("gcc", "/", "68020", "Os", source, "p.o", False, [])
    plain = tmp_path / "q.c"
    plain.write_text("int a;\n")
    assert "-D__clang__" not in m68k_clk.gcc_command("gcc", "/", "68020", "Os", plain, "p.o", False, [])


def test_the_bytes_of_a_cell_are_text_plus_data():
    """A static table that is not const sits in `.data`; the text column alone gave word_compare_branches 208 B on both sides, 264 B loaded."""
    from m68k_clk import loaded_bytes

    assert loaded_bytes("   text\t   data\t    bss\t    dec\t    hex\tfilename\n    208\t     56\t      0\t    264\t    108\to.o\n") == "264"


def test_gcc_is_hosted_like_ours():
    """m32_word_vectors -O1 68000: ours (hosted since #10561) called the stub memset for its fused fill loop while gcc, built `-ffreestanding`, inlined the fills and never referenced the stub: the pair compared a libc call with an inline store."""
    import m68k_clk

    command = m68k_clk.gcc_command("gcc", "/", "68000", "O1", "p.c", "p.o", True)
    assert "-ffreestanding" not in command, command


def test_gcc_takes_the_shared_external_flags():
    """The clock column's gcc was built with distro defaults (no -fomit-frame-pointer, stack protector and unwind tables on): every clock bar of the m68k census was against a gcc that is not the bar."""
    import external_flags
    import m68k_clk

    command = m68k_clk.gcc_command("gcc", "/", "68000", "Os", "p.c", "p.o", False)
    for flag in external_flags.flags("none", (), 15):
        assert flag in command, (flag, command)


def _fake_cycles(path, stamp):
    path.write_text(f"#!/bin/sh\necho {stamp}\n")
    path.chmod(0o755)


def test_a_cycles_binary_outside_the_checkouts_target_dir_is_refused(tmp_path):
    """The census priced 1,374 cells with another session's `cycles` build from before #10557: `>=` where the tip says `<=`, so cells read undecidable."""
    import pytest
    import m68k_clk

    elsewhere, target = tmp_path / "w3", tmp_path / "mine"
    elsewhere.mkdir(), target.mkdir()
    _fake_cycles(elsewhere / "cycles", m68k_clk.sources_stamp(m68k_clk.ROOT))
    with pytest.raises(SystemExit, match="outside this checkout's target dir"):
        m68k_clk.verify_cycles(elsewhere / "cycles", m68k_clk.ROOT, target)


def test_a_cycles_binary_built_from_other_sources_is_refused(tmp_path):
    import pytest
    import m68k_clk

    _fake_cycles(tmp_path / "cycles", "0000000000000000")
    with pytest.raises(SystemExit, match="other llrm-m68k sources"):
        m68k_clk.verify_cycles(tmp_path / "cycles", m68k_clk.ROOT, tmp_path)


def test_a_cycles_binary_of_this_checkouts_sources_is_accepted(tmp_path):
    import m68k_clk

    _fake_cycles(tmp_path / "cycles", m68k_clk.sources_stamp(m68k_clk.ROOT))
    m68k_clk.verify_cycles(tmp_path / "cycles", m68k_clk.ROOT, tmp_path)
