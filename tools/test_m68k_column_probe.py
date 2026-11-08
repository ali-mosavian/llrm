"""A column that names a hardening flag or a PIC model, or an object with a GOT/PLT kind, is refused."""
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_column_probe import column_flags, object_violations, reloc_kinds, violations  # noqa: E402

SOURCE = '''
fn gcc_object() {
    .args(["-ffreestanding", "-nostdlib", "-fno-asynchronous-unwind-tables"])
    .arg(format!("-mcpu={cpu}"))
}
fn clang_object() {
    .args(["--target=m68k-linux-gnu", "-ffreestanding", "-fno-asynchronous-unwind-tables", "-fno-pic"])
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

    def test_got_and_plt_kinds_are_refused_and_pc_kinds_are_not(self):
        kinds = reloc_kinds("00000a  R_68K_PC16  .bss\n00000c  R_68K_GOT16O  x\n000010  R_68K_PLT32  f\n000014  R_68K_32  g")
        self.assertEqual(object_violations(kinds), ["R_68K_GOT16O", "R_68K_PLT32"])


if __name__ == "__main__":
    unittest.main()
