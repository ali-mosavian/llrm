"""The runtime's scratch cells as zero page: __mulsi3 was 83 B with three-byte addresses, 65 B with two-byte ones."""
import subprocess, sys, tempfile, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import mkrt


def code_bytes(source):
    with tempfile.TemporaryDirectory() as tmp:
        Path(tmp, "m.s").write_text(source)
        subprocess.run(["ca65", "-t", "sim6502", "-o", "m.o", "m.s"], cwd=tmp, check=True, capture_output=True)
        out = subprocess.run(["od65", "--dump-segsize", "m.o"], cwd=tmp, check=True, capture_output=True, text=True).stdout
    return int(next(l for l in out.splitlines() if "CODE:" in l).split()[1])


class ScratchInZeroPage(unittest.TestCase):
    rt = mkrt.MOS_RT.read_text()

    def test_the_multiply_is_18_bytes_shorter_with_the_scratch_in_zero_page(self):
        bss = code_bytes(mkrt.split_rt(self.rt)["__mulsi3"])
        zp = code_bytes(mkrt.split_rt(self.rt, scratch_zp=True)["__mulsi3"])
        self.assertEqual((bss, zp), (83, 65))

    def test_the_cells_member_is_zeropage_and_the_others_import_it_as_such(self):
        members = mkrt.split_rt(self.rt, scratch_zp=True)
        self.assertIn('.segment "ZEROPAGE"', members["__rt_cells"])
        self.assertIn(".exportzp __sa", members["__rt_cells"])
        self.assertIn(".importzp __sa", members["__mulsi3"])

    def test_the_sim65_row_has_the_bytes(self):
        self.assertGreaterEqual(mkrt.scratch_zero_page("sim65-cc65"), mkrt.SCRATCH_CELLS)
        self.assertEqual(mkrt.scratch_zero_page("c64"), 0)


if __name__ == "__main__":
    unittest.main()
