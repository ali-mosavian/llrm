"""`uv run --project tools python -m unittest discover -s crates/target/llrm-x86-m16/bench -p "test_icount.py"`"""

import os
import sys
import tempfile
import subprocess
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
REPO = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO / "tools" / "dosbatch"))
sys.path.insert(0, str(REPO / "tools" / "bench"))

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

    def test_only_the_code_named_is_counted_and_the_region_still_ends_at_its_return(self):
        """A scene around the code under test (d_faces' stand-ins) was counted with it: `only` limits the count to a range of the
        image, and the `ret` that ends the region is outside the range here."""
        entry = maps.find(maps.symbols(self.listing), "region")
        # `mov cx, 5` and `dec cx` (4 bytes) are 1 + 5 instructions; the rest of the region is outside.
        result = icount.run(self.exe, entry, self.directory, only=(entry, entry + 4))
        self.assertEqual(result.error, "")
        self.assertEqual(result.region.instructions, 6)

    def test_a_region_costs_what_its_instructions_add_up_to(self):
        """The first count read 155 for this region: a REP fires the hook once per iteration and the count also multiplied by CX."""
        entry = maps.find(maps.symbols(self.listing), "region")
        result = icount.run(self.exe, entry, self.directory)
        self.assertEqual(result.error, "")
        self.assertEqual((result.region.instructions, result.region.memory_operands), (34, 34))

    def test_a_region_costs_the_486s_clocks_by_vsgccs_table(self):
        """1 + 5 dec + 4 jnz taken at 3 and one not at 1 + 3 movs and cld at 1 + the rep movsw (15 for the first,
        3 for each of the 15 after) + 2 movs + a ret at 5 = 90. A branch taken costs more than one not taken: the
        stream's price, not the count's."""
        entry = maps.find(maps.symbols(self.listing), "region")
        result = icount.run(self.exe, entry, self.directory)
        self.assertEqual(result.region.clocks, 1 + 5 + (4 * 3 + 1) + 4 + (15 + 15 * 3) + 2 + 5)

    def test_a_rep_with_nothing_to_repeat_costs_nothing(self):
        """The hook still fires once with CX at 0: 4 instructions in `other`, not 5, and no memory operand."""
        entry = maps.find(maps.symbols(self.listing), "other")
        result = icount.run(self.exe, entry, self.directory)
        self.assertEqual((result.region.instructions, result.region.memory_operands), (3 + 1, 0))

    def test_a_whole_program_is_counted_from_its_first_instruction_to_its_exit(self):
        """A program with no region to name (tests/run's, for the clocks sweep) counted nothing: `entry=None` only ran it.
        `whole` counts the 34 of `region`, the 4 of `other` and the 7 of the start-up, the exit's `int 21h` included."""
        result = icount.run(self.exe, None, self.directory, whole=True)
        self.assertEqual((result.error, result.exit_code), ("", 0))
        self.assertEqual(result.region.instructions, 34 + 4 + 7)

    def test_a_profile_counts_each_address_as_often_as_it_ran(self):
        """A hot loop in grep.bas (2.1x the clocks of main) was found by reading two listings: `profile` names the addresses that ran, and how often."""
        entry = maps.find(maps.symbols(self.listing), "region")
        profile = {}
        icount.run(self.exe, None, self.directory, whole=True, profile=profile)
        # `mov cx, 5` (3 bytes), then `dec cx`: the loop body runs five times
        self.assertEqual((profile[entry], profile[entry + 3]), (1, 5))

    def test_a_function_the_program_never_calls_is_reported_not_counted_as_zero(self):
        result = icount.run(self.exe, 0x7000, self.directory)
        self.assertIsNone(result.region)
        self.assertIn("never entered", result.error)


EXIT_WITH = """\
.model small
.386
.stack 256
.data
{data}
.code
start:
    mov ax, @data
    mov ds, ax
{body}
    mov ah, 4Ch
    int 21h
end start
"""


@unittest.skipUnless((BIN / "jwasm").exists(), "needs jwasm and jwlink beside llrm's binaries (cargo build)")
class ServiceTests(unittest.TestCase):
    def run_body(self, body: str, file: bytes = b"", data: str = "dummy db 0"):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            (directory / "S.ASM").write_text(EXIT_WITH.format(body=body, data=data))
            (directory / "DATA.BIN").write_bytes(file)
            subprocess.run([str(BIN / "jwasm"), "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{directory}/S.OBJ", str(directory / "S.ASM")], check=True)
            subprocess.run([str(BIN / "jwlink"), "option", "quiet", "format", "dos", "name", str(directory / "S.EXE"), "file", str(directory / "S.OBJ")], check=True)
            return icount.run(directory / "S.EXE", None, directory, whole=True)

    def test_a_program_that_asks_for_the_current_drive_runs_to_its_exit(self):
        """`int 21h ah=19h` (QuickBASIC's startup, grep.bas and stft_single.bas) stopped the run with `int 21h ah=0x19`: those
        programs had no clock row. The drive is C: (2) and AL carries it to the exit code."""
        result = self.run_body("    mov ah, 19h\n    int 21h\n    mov bl, al\n    mov al, bl")
        self.assertEqual((result.error, result.exit_code), ("", 2))

    def test_a_file_opened_by_its_drive_and_root_path_is_found_in_the_programs_directory(self):
        """QuickBASIC names a file `C:\\DATA.BIN` once the drive is known: the emulator looked for a file of that very name, opened
        nothing and the program printed 0 where stft_single.bas and grep.bas want 78803551 and 4745."""
        body = """    mov dx, offset fname
    mov ax, 3D00h
    int 21h
    jc fail
    mov bx, ax
    mov dx, offset buf
    mov cx, 1
    mov ah, 3Fh
    int 21h
    mov al, buf
    jmp done
fail:
    mov al, 99
done:"""
        result = self.run_body(body, bytes([7]), "fname db 'C:\\DATA.BIN', 0\nbuf db 0")
        self.assertEqual((result.error, result.exit_code), ("", 7))


class MapTests(unittest.TestCase):
    def test_a_name_matches_as_each_language_spells_it(self):
        table = {"_bench_sieve": 24, "BENCHSIEVE": 86, "BENCHFRAMES&": 9}
        self.assertEqual(maps.find(table, "bench_sieve"), 24)
        self.assertEqual(maps.find(table, "BenchFrames"), 9)
        self.assertIsNone(maps.find(table, "bench_crc"))


if __name__ == "__main__":
    unittest.main()



def test_a_dos_name_finds_the_file_of_another_case(tmp_path):
    """stft_fixed opens SIGNAL and the corpus file is `signal`: the emulator opened nothing and the program printed -1."""
    (tmp_path / "signal").write_bytes(b"x")
    assert icount.dos_file(tmp_path, "SIGNAL") == tmp_path / "signal"
    assert icount.dos_file(tmp_path, "C:\\SIGNAL") == tmp_path / "signal"
    assert icount.dos_file(tmp_path, "NEW") == tmp_path / "NEW"


@unittest.skipUnless((BIN / "jwasm").exists(), "needs jwasm and jwlink beside llrm's binaries (cargo build)")
def test_a_program_that_never_resets_the_fpu_starts_at_dos_control_word(tmp_path):
    """Unicorn starts with FPCW 0 (single precision): stft_single nib, which has no fninit, printed 204598972 for DOSBox's 78803551."""
    program = (
        ".model small\n.386\n.387\n.stack 256\n.data\ncw dw 0\n.code\nstart:\n    mov ax, @data\n    mov ds, ax\n"
        "    fnstcw [cw]\n    mov ah, 40h\n    mov bx, 1\n    mov cx, 2\n    mov dx, offset cw\n    int 21h\n    mov ax, 4C00h\n    int 21h\nend start\n"
    )
    (tmp_path / "F.ASM").write_text(program)
    subprocess.run([str(BIN / "jwasm"), "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path}/F.OBJ", str(tmp_path / "F.ASM")], check=True)
    subprocess.run([str(BIN / "jwlink"), "option", "quiet", "format", "dos", "name", str(tmp_path / "F.EXE"), "file", str(tmp_path / "F.OBJ")], check=True)
    assert icount.run(tmp_path / "F.EXE", None, tmp_path, whole=True).output == b"\x7f\x03"
