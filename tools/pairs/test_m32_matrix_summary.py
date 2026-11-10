"""A row with an empty ours/pin_clang cell is reported and fails the summary (z80's table took the next-best column and understated gaps).
A size is code + data: a table in .rodata made ours read better than a reference built from immediates (text-only matrix, until 2026-11-09)."""
import subprocess, sys, tempfile, unittest
from pathlib import Path

BASE = "target program ours_padded pin_llc_padded pin_clang_padded ours_code pin_llc_code pin_clang_code x x x x x x note gcc_padded gcc_code".split()
DATA = "ours_data ours_bss pin_llc_data pin_llc_bss pin_clang_data pin_clang_bss gcc_data gcc_bss".split()


def tsv(columns, **cells):
    """One x86_32-linux row of program p: every code column 10, every data/bss column 0, `cells` overrides by name; None blanks it."""
    row = {c: "" for c in columns}
    row.update(target="x86_32-linux", program="p", **{c: "10" for c in columns if c.endswith("_code")}, **{c: "0" for c in DATA if c in columns})
    row.update({k: "" if v is None else str(v) for k, v in cells.items()})
    return "\t".join(columns) + "\n" + "\t".join(row[c] for c in columns) + "\n"


class T(unittest.TestCase):
    def run_summary(self, text):
        d = Path(tempfile.mkdtemp()); stamp = d / "stamp.txt"; stamp.write_text("s")
        f = d / "m-x-O1-2026.tsv"; f.write_text(text)
        return subprocess.run([sys.executable, str(Path(__file__).parent / "m32_matrix_summary.py"), str(stamp), str(f)], capture_output=True, text=True)

    def test_missing_pin_clang_fails(self):
        r = self.run_summary(tsv(BASE + DATA, pin_clang_code=None))
        self.assertEqual(r.returncode, 1); self.assertIn("MISSING", r.stderr)

    def test_missing_gcc_fails(self):
        r = self.run_summary(tsv(BASE + DATA, gcc_code=None))
        self.assertEqual(r.returncode, 1); self.assertIn("gcc_code", r.stderr)

    def test_an_all_na_pin_llc_column_fails_not_reads_as_no_gap(self):
        """The O2 matrix of 11-09 had pin_llc NA on 349 of 349 cells (a call passed minsize= to a pin.target that has no such argument) and the summary printed 0 worse vs pin llc."""
        r = self.run_summary(tsv(BASE + DATA, pin_llc_code=None))
        self.assertEqual(r.returncode, 1); self.assertIn("pin_llc_code", r.stderr)

    def test_an_x86_64_row_with_no_a4_or_gcc_cell_is_not_missing(self):
        """The a4 and gcc columns are filled for x86_32 rows only; choosing the clang column per file made every x86_64 row of a mixed file MISSING (exit 1)."""
        a4 = ["pin_clang_a4_code", "pin_clang_a4_data", "pin_clang_a4_bss", "pin_clang_a4_padded"]
        r = self.run_summary(tsv(BASE + DATA + a4, target="x86_64-linux", gcc_code=None, pin_clang_a4_code=None))
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_complete_row_passes(self):
        self.assertEqual(self.run_summary(tsv(BASE + DATA)).returncode, 0)

    def test_a_file_without_data_columns_is_refused_not_read_as_text_only(self):
        """A text-only file sized every cell as code and said nothing."""
        r = self.run_summary(tsv(BASE))
        self.assertEqual(r.returncode, 1); self.assertIn("ours_data", r.stderr)

    def test_an_unreadable_data_cell_is_missing_not_zero(self):
        r = self.run_summary(tsv(BASE + DATA, gcc_data=None))
        self.assertEqual(r.returncode, 1); self.assertIn("gcc_data", r.stderr)

    def test_a_table_in_rodata_is_worse_by_size_and_listed_as_a_flip(self):
        """ours 8 B of code + 64 B table against 30 B of immediates: code alone says better (8 < 30), code + data says worse (72 > 30)."""
        r = self.run_summary(tsv(BASE + DATA, ours_code=8, ours_data=64, pin_clang_code=30, gcc_code=30, pin_llc_code=30))
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("flip\tO1\tx86_32-linux\tp\tworse\tcode 8/30 -> size 72/30", r.stdout)
        self.assertIn("1 newly worse, 0 newly better", r.stdout)
        row = [l for l in r.stdout.splitlines() if l.startswith("x\t") or l.split("\t")[:2] == ["O1", "x86_32-linux"]][0].split("\t")
        self.assertEqual((row[5], row[6], row[9], row[10]), ("1", "1", "1", "1"))  # llc, clang, gcc, best: all worse by size

    def test_a_table_dropped_from_data_to_immediates_flips_better(self):
        r = self.run_summary(tsv(BASE + DATA, ours_code=40, pin_clang_code=30, pin_clang_data=64, gcc_code=30, gcc_data=64))
        self.assertIn("flip\tO1\tx86_32-linux\tp\tbetter\tcode 40/30 -> size 40/94", r.stdout)

    def test_extra_bss_is_worse_only_as_static(self):
        r = self.run_summary(tsv(BASE + DATA, ours_bss=20))
        row = [l for l in r.stdout.splitlines() if l.split("\t")[:2] == ["O1", "x86_32-linux"]][0].split("\t")
        self.assertEqual((row[10], row[13]), ("0", "1"))  # size not worse, static worse

    def test_a_column_whose_code_exceeds_its_emitted_text_is_refused(self):
        """m64 tail_calls O1 read ours 461 code + 84 pad against gcc 512 + 0: a padded and an unpadded column compared. A code column larger
        than the text its object emitted is no padding-free count; the summary exits 2."""
        r = self.run_summary(tsv(BASE + DATA, ours_padded=9, ours_code=10))
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        self.assertIn("REFUSED code_bytes padding-free", r.stderr)


if __name__ == "__main__":
    unittest.main()
