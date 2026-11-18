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
        zp = code_bytes(mkrt.split_rt(self.rt, scratch_at=0x9A)["__mulsi3"])
        self.assertEqual((bss, zp), (83, 65))

    def test_the_cells_are_equates_after_the_registers_and_no_member_holds_them(self):
        members = mkrt.split_rt(self.rt, scratch_at=0x9A)
        self.assertNotIn("__rt_cells", members)
        head = members["__mulsi3"]
        for line in ("__sa = $9A", "__ra = $9E", "__st = $A0", "__sr = $A4", "__sg = $A8"):
            self.assertIn(line, head)

    def test_the_rows_with_room_place_the_scratch_after_the_registers(self):
        self.assertEqual(mkrt.scratch_address("sim65-cc65"), 0x9A)
        self.assertEqual(mkrt.scratch_address("atari8"), 0x9C)
        self.assertIsNone(mkrt.scratch_address("c64"))


if __name__ == "__main__":
    unittest.main()
