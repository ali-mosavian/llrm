"""`uv run --project tools python -m unittest discover -s tools/bench`"""

import os
import sys
import tempfile
import subprocess
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "dosbatch"))

import maps  # noqa: E402
import icount  # noqa: E402
from dosbatch import BIN  # noqa: E402

# A region of known cost: 1 + 5*2 + 4 + 16 (the REP) + 3 = 34 instructions, 2*16 + 2 = 34 memory operands.
PROGRAM = """\
.model small
.386
.stack 256
.data
src dw 16 dup (1)
dst dw 16 dup (0)
.code
region proc near
    mov cx, 5
l1: dec cx
    jnz l1
    mov cx, 16
    mov si, offset src
    mov di, offset dst
    cld
    rep movsw
    mov ax, [src]
    mov [dst], ax
    ret
region endp
other proc near
    mov cx, 0
    mov si, offset src
    mov di, offset dst
    rep movsw
    ret
other endp
start:
    mov ax, @data
    mov ds, ax
    mov es, ax
    call region
    call other
    mov ax, 4C00h
    int 21h
end start
"""


def build(directory: Path) -> tuple[Path, Path]:
    (directory / "T.ASM").write_text(PROGRAM)
    subprocess.run([str(BIN / "jwasm"), "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{directory}/T.OBJ", str(directory / "T.ASM")], check=True)
    subprocess.run([str(BIN / "jwlink"), "option", "quiet", f"option", f"map={directory}/T.MAP", "format", "dos", "name", str(directory / "T.EXE"), "file", str(directory / "T.OBJ")], check=True)
    return directory / "T.EXE", directory / "T.MAP"


@unittest.skipUnless((BIN / "jwasm").exists(), "needs jwasm and jwlink beside llrm's binaries (cargo build)")
class CountTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.directory = Path(self.temp.name)
        self.exe, self.listing = build(self.directory)

    def tearDown(self):
        self.temp.cleanup()

    def test_a_region_costs_what_its_instructions_add_up_to(self):
        """The first count read 155 for this region: a REP fires the hook once per iteration and the count also multiplied by CX."""
        entry = maps.find(maps.symbols(self.listing), "region")
        result = icount.run(self.exe, entry, self.directory)
        self.assertEqual(result.error, "")
        self.assertEqual((result.region.instructions, result.region.memory_operands), (34, 34))

    def test_a_rep_with_nothing_to_repeat_costs_nothing(self):
        """The hook still fires once with CX at 0: 4 instructions in `other`, not 5, and no memory operand."""
        entry = maps.find(maps.symbols(self.listing), "other")
        result = icount.run(self.exe, entry, self.directory)
        self.assertEqual((result.region.instructions, result.region.memory_operands), (3 + 1, 0))

    def test_a_function_the_program_never_calls_is_reported_not_counted_as_zero(self):
        result = icount.run(self.exe, 0x7000, self.directory)
        self.assertIsNone(result.region)
        self.assertIn("never entered", result.error)


class MapTests(unittest.TestCase):
    def test_a_name_matches_as_each_language_spells_it(self):
        table = {"_bench_sieve": 24, "BENCHSIEVE": 86, "BENCHFRAMES&": 9}
        self.assertEqual(maps.find(table, "bench_sieve"), 24)
        self.assertEqual(maps.find(table, "BenchFrames"), 9)
        self.assertIsNone(maps.find(table, "bench_crc"))


if __name__ == "__main__":
    unittest.main()
