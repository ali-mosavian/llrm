"""A column that names a hardening flag or a PIC model, or an object with a GOT/PLT kind, is refused."""
import os
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_column_probe import ARRAY_SOURCE, FRAME_SOURCE, column_flags, flags_change_gcc_output, padding_flag_reaches_gcc, object_violations, reloc_kinds, shared_violations, violations  # noqa: E402

def gcc_driver() -> str:
    """The unpacked cross gcc, or a named skip: without GCC_M68K these tests died with a bare KeyError."""
    root = os.environ.get("GCC_M68K")
    if not root:
        raise unittest.SkipTest("GCC_M68K unset: needs the unpacked m68k gcc (see tools/m68k_column_probe.py)")
    return os.path.join(root, "usr/bin/m68k-linux-gnu-gcc-15")


SOURCE = '''
const EXTERNAL_FLAGS: [&str; 5] = [
    "-fomit-frame-pointer",
    "-fno-stack-protector",
    "-fno-stack-clash-protection",
    "-U_FORTIFY_SOURCE",
    "-fno-asynchronous-unwind-tables",
];
const GCC_ZERO_INIT_PADDING: &str = "-fzero-init-padding-bits=standard";
fn gcc_object() {
    .args(["-ffreestanding", "-nostdlib"]).args(EXTERNAL_FLAGS).arg(GCC_ZERO_INIT_PADDING)
    .arg(format!("-mcpu={cpu}"))
}
fn clang_object() {
    .args(["--target=m68k-linux-gnu", "-ffreestanding", "-fno-pic"]).args(EXTERNAL_FLAGS)
    .args(["-mtriple=m68k-linux-gnu", "-relocation-model=static", "-O2", "-filetype=obj"])
}
'''


class ColumnProbe(unittest.TestCase):
    def test_the_matrix_flags_as_written_are_neutral(self):
        self.assertEqual(violations(column_flags(SOURCE)), [])

    def test_a_hardening_flag_or_pic_in_a_column_is_named(self):
        flags = column_flags(SOURCE.replace('"-nostdlib"', '"-nostdlib", "-fstack-protector-strong", "-fPIE"'))
        self.assertEqual(len(violations(flags)), 2)

    def test_a_clang_column_without_the_static_model_is_refused(self):
        flags = column_flags(SOURCE.replace('"-fno-pic"', '"-O0"').replace("-relocation-model=static", "-O1"))
        self.assertEqual(len(violations(flags)), 2)

    def test_the_fixture_matches_the_shared_config_and_a_missing_flag_or_constant_is_named(self):
        self.assertEqual(shared_violations(SOURCE), [])
        self.assertEqual(len(shared_violations(SOURCE.replace('    "-fomit-frame-pointer",\n', ""))), 1)
        self.assertEqual(len(shared_violations(SOURCE.replace("standard", "all"))), 1)
        self.assertEqual(shared_violations(SOURCE.replace(".arg(GCC_ZERO_INIT_PADDING)", "")), ["gcc_object: does not pass GCC_ZERO_INIT_PADDING"])

    def test_got_and_plt_kinds_are_refused_and_pc_kinds_are_not(self):
        kinds = reloc_kinds("00000a  R_68K_PC16  .bss\n00000c  R_68K_GOT16O  x\n000010  R_68K_PLT32  f\n000014  R_68K_32  g")
        self.assertEqual(object_violations(kinds), ["R_68K_GOT16O", "R_68K_PLT32"])

    def test_the_zero_init_padding_flag_changes_gcc_output_so_a_zero_diff_is_a_result(self):
        """A 92-cell A/B showed 0 differing cells; this source differs, so the 0 was not an inert flag."""
        gcc = gcc_driver()
        self.assertTrue(padding_flag_reaches_gcc(gcc))

    def test_the_opposite_of_a_shared_flag_changes_the_bytes_of_one_cell(self):
        """11-09: the shared flags left every common cell byte-identical; a flag that never arrived would read the same."""
        gcc = gcc_driver()
        self.assertTrue(flags_change_gcc_output(gcc, ARRAY_SOURCE, ["-fno-stack-protector"], ["-fstack-protector-strong"]))
        self.assertTrue(flags_change_gcc_output(gcc, FRAME_SOURCE, ["-fomit-frame-pointer"], ["-fno-omit-frame-pointer"]))
        self.assertFalse(flags_change_gcc_output(gcc, FRAME_SOURCE, ["-fomit-frame-pointer"], ["-fomit-frame-pointer"]), "control: equal flags")

    def test_an_unset_gcc_m68k_is_a_named_skip_not_a_key_error(self):
        saved = os.environ.pop("GCC_M68K", None)
        try:
            with self.assertRaises(unittest.SkipTest) as raised:
                gcc_driver()
            self.assertIn("GCC_M68K", str(raised.exception))
        finally:
            if saved is not None:
                os.environ["GCC_M68K"] = saved


if __name__ == "__main__":
    unittest.main()
