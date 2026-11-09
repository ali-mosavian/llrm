"""Code bytes of an object, never the file's size: an unreadable object is an error (exit 2), not `stat -c %s`."""
import subprocess
import sys
from pathlib import Path

import pytest

TOOL = Path(__file__).with_name("object_bytes.py")


def run(target: str, obj: Path) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(TOOL), target, str(obj)], capture_output=True, text=True)


def test_an_object_that_is_not_the_targets_format_is_an_error_not_its_file_size(tmp_path):
    """cells.sh read 4-6x the code for OMF, z80 and 6502 objects by falling back to the file's bytes."""
    junk = tmp_path / "o.o"
    junk.write_bytes(b"not an object " * 100)
    for target in ("x86_16-wat", "x86_32-linux", "m68k-68020", "z80-sdcc", "6502-sim65", "arm64-linux", "wasm32"):
        done = run(target, junk)
        assert done.returncode == 2 and done.stdout == "", (target, done.stdout, done.stderr)


def test_a_z80_object_counts_its_assembled_code_not_the_text(tmp_path):
    source = tmp_path / "o.s"
    source.write_text("\t.area _CODE\n_f::\n\tld hl, #5\n\tret\n" + "; a long comment that is no code\n" * 40)
    done = run("z80-sdcc", source)
    assert done.returncode == 0 and done.stdout.split() == ["4", "0", "0"], done.stderr
    assert source.stat().st_size > 1000


def test_a_z80_object_counts_its_data_areas_apart(tmp_path):
    source = tmp_path / "o.s"
    source.write_text("\t.area _CODE\n_f::\n\tret\n\t.area _CONST\n_t::\n\t.db 1,2,3,4,5\n")
    done = run("z80-sdcc", source)
    assert done.stdout.split() == ["1", "5", "0"], done.stderr


def test_an_omf_object_counts_code_and_data_not_the_file(tmp_path):
    """The m16 fixture: 4327 file bytes hold 1135 of code and 333 of data (the rest are THEADR, LNAMES, fixups, symbols)."""
    pytest.importorskip("iced_x86")
    obj = Path(__file__).resolve().parents[2] / "tests/fixtures/bench/fpbench-v-g3.obj"
    done = run("x86_16-wat", obj)
    assert done.returncode == 0 and done.stdout.split() == ["1135", "333", "3"], done.stderr
    assert obj.stat().st_size > 4000


def test_an_elf_object_counts_text_and_data_sections(tmp_path):
    c = tmp_path / "o.c"
    c.write_text("int table[10] = {1}; const char k[7] = \"abcdef\"; int zero[20]; int f(void) { return table[1]; }\n")
    obj = tmp_path / "o.o"
    subprocess.run(["cc", "-O1", "-c", str(c), "-o", str(obj)], check=True)
    done = run("x86_32-linux", obj)
    code, data, align = (int(n) for n in done.stdout.split())
    assert code > 0 and data >= 40 + 7 + 80, done.stdout
    assert align >= 0 and data >= 40 + 7 + 80, "data counts the bss buffer: `zero[20]` is 80 B of it"
    assert code + align == sum(int(l.split()[1]) for l in subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True).stdout.splitlines() if l.startswith(".text")), "code + align is the .text section"


def test_a_6502_object_counts_the_assembled_code_segment(tmp_path):
    source = tmp_path / "o.s"
    source.write_text("\t.segment \"CODE\"\n_f:\n\tlda #5\n\trts\n" + "; padding comment\n" * 40)
    done = run("6502-sim65", source)
    assert done.returncode == 0 and done.stdout.split() == ["3", "0", "0"], done.stderr


def test_a_missing_or_empty_object_is_an_error(tmp_path):
    empty = tmp_path / "e.o"
    empty.write_bytes(b"")
    assert run("x86_32-linux", empty).returncode == 2
    assert run("x86_32-linux", tmp_path / "absent.o").returncode == 2


def test_an_x86_elf_code_column_leaves_the_alignment_padding_out_and_reports_it_apart(tmp_path):
    """Two figures went by one name: the repo reader's ELF CODE was the `.text*` section size with alignment padding, the harness columns (m32 code_bytes,
    m64 code_size) padding-free. Two functions 16-aligned: 5 B of code, 11 of nop fill, 1 B of code."""
    source = tmp_path / "a.s"
    source.write_text(".text\n.globl f, g\nf: movl 4(%esp), %eax\nret\n.balign 16\ng: ret\n")
    obj = tmp_path / "a.o"
    subprocess.run(["as", "--32", str(source), "-o", str(obj)], check=True)
    section = sum(int(line.split()[1]) for line in subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True).stdout.splitlines() if line.startswith(".text"))
    code, data, align = (int(n) for n in run("x86_32-linux", obj).stdout.split())
    assert section == 17, "premise: the section is 17 B with the padding"
    assert (code, align) == (6, 11), (code, data, align)
    assert code + align == section


def test_a_big_endian_m68k_elf_code_column_is_the_section_size_and_align_is_zero(tmp_path):
    """arm64 and m68k padding hides in the section like x86's, and elfsize reads little-endian only: their CODE is `size -A`'s `.text` WITH padding, ALIGN 0.
    A real m68k object: 2 B of code, .balign 16, 2 B more; a 4 B constant, a 20 B table, 8 B of bss."""
    source = tmp_path / "m.s"
    source.write_text(".text\nf: rts\n.balign 16\ng: rts\n.section .rodata\nk: .long 1\n.data\nt: .fill 20,1,0\n.bss\n.lcomm z, 8\n")
    obj = tmp_path / "m.o"
    if subprocess.run(["m68k-linux-gnu-as", str(source), "-o", str(obj)], capture_output=True).returncode:
        pytest.skip("no m68k assembler")
    listing = subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True, check=True).stdout
    section = {row.split()[0]: int(row.split()[1]) for row in listing.splitlines() if len(row.split()) > 2 and row.split()[1].isdigit()}
    assert section[".text"] == 18, "premise: 2 + 14 of padding + 2"
    code, data, align = (int(n) for n in run("m68k-68020", obj).stdout.split())
    assert (code, data, align) == (18, 4 + 20 + 8, 0), (code, data, align)


def test_an_x86_64_object_with_a_bss_buffer_a_const_table_and_an_aligned_loop(tmp_path):
    """m64's old column read 0 for the bss (histo 0 where it is 4608). static char bss[4096] and const int table[16] give data + bss >= 4160, and a loop aligned to
    32 gives ALIGN > 0 that CODE leaves out."""
    c = tmp_path / "m.c"
    c.write_text("static char bss[4096]; const int table[16] = {1};\nint f(int n) { int s = 0; for (int i = 0; i < n; i++) { bss[i & 4095] = (char)i; s += table[i & 15] + bss[(i * 7) & 4095]; } return s; }\n")
    obj = tmp_path / "m.o"
    subprocess.run(["gcc", "-O2", "-falign-loops=32", "-fno-pie", "-c", str(c), "-o", str(obj)], check=True)
    section = sum(int(line.split()[1]) for line in subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True).stdout.splitlines() if line.startswith(".text"))
    code, data, align = (int(n) for n in run("x86_64-linux", obj).stdout.split())
    assert data >= 4096 + 64, (code, data, align)
    assert align > 0 and code + align == section, (code, align, section)


def test_object_bytes_reads_an_elf_without_importing_the_compiler_tooling(tmp_path):
    """`import sizes` pulls llrmbin, benchlist and run_tests, which look for compiler binaries: an ELF size must not need them (m64's review of #8533)."""
    source = tmp_path / "a.s"
    source.write_text(".text\nf: ret\n")
    obj = tmp_path / "a.o"
    subprocess.run(["as", "--32", str(source), "-o", str(obj)], check=True)
    probe = "import runpy, sys; sys.argv = ['o', 'x86_32-linux', %r]; sys.path.insert(0, %r)\ntry: runpy.run_path(%r, run_name='__main__')\nexcept SystemExit: pass\nprint('sizes' in sys.modules, 'llrmbin' in sys.modules)" % (str(obj), str(TOOL.parent), str(TOOL))
    done = subprocess.run([sys.executable, "-c", probe], capture_output=True, text=True)
    assert done.stdout.split()[-2:] == ["False", "False"], done.stdout + done.stderr
