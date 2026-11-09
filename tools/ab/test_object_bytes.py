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


def test_a_non_x86_elf_code_column_keeps_the_section_size_and_prints_align_zero():
    """m68k is big-endian; elfsize reads little-endian only, so its CODE is the section size, ALIGN 0, as the module docstring says."""
    import object_bytes

    assert object_bytes.Footprint(10, 0, 0).align == 0
    assert "WITH padding" in object_bytes.__doc__
