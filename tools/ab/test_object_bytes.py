"""Code bytes of an object, never the file's size: an unreadable object is an error (exit 2), not `stat -c %s`."""
import os
import subprocess
import sys
from pathlib import Path

import pytest

TOOL = Path(__file__).with_name("object_bytes.py")


@pytest.fixture
def needs_binaries():
    """ELF and OMF objects are read through `sizes`, which imports dosbatch: it refuses a tree whose release binaries are stale."""
    sys.path.insert(0, str(TOOL.parents[1]))
    try:
        import sizes  # noqa: F401
    except SystemExit as refused:
        pytest.skip(str(refused))


def run(target: str, obj: Path) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(TOOL), target, str(obj)], capture_output=True, text=True)


def test_an_object_that_is_not_the_targets_format_is_an_error_not_its_file_size(tmp_path):
    """cells.sh read 4-6x the code for OMF, z80 and 6502 objects by falling back to the file's bytes."""
    junk = tmp_path / "o.o"
    junk.write_bytes(b"not an object " * 100)
    for target in ("z80-sdcc", "6502-sim65", "65c02-sim65", "wasm32"):
        done = run(target, junk)
        assert done.returncode == 2 and done.stdout == "", (target, done.stdout, done.stderr)


def test_an_elf_or_omf_target_refuses_junk_too(tmp_path, needs_binaries):
    junk = tmp_path / "o.o"
    junk.write_bytes(b"not an object " * 100)
    for target in ("x86_16-wat", "x86_32-linux", "m68k-68020", "arm64-linux"):
        done = run(target, junk)
        assert done.returncode == 2 and done.stdout == "", (target, done.stdout, done.stderr)


def test_z80_6502_and_wasm_objects_are_read_in_a_tree_with_stale_binaries(tmp_path):
    """The reader imported `sizes` -> dosbatch at load, which refuses a tree with stale release binaries: unit tests of the z80, 6502 and wasm readers died with `stale binaries`
    (exit 1, no bytes) in any worktree that had built once and then pulled."""
    stale = tmp_path / "bin"
    stale.mkdir()
    for name in ("llrm-cc", "llrm-llc"):
        (stale / name).write_bytes(b"")
        os.utime(stale / name, (0, 0))
    source = tmp_path / "o.s"
    source.write_text("\t.area _CODE\n_f::\n\tret\n")
    done = subprocess.run([sys.executable, str(TOOL), "z80-sdcc", str(source)], capture_output=True, text=True, env={**os.environ, "LLRM_BIN": str(stale)})
    assert done.returncode == 0 and done.stdout.split() == ["1", "0", "0"], done.stderr


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


def test_an_omf_object_counts_code_and_data_not_the_file(tmp_path, needs_binaries):
    """The m16 fixture: 4327 file bytes hold 1135 of code and 333 of data (the rest are THEADR, LNAMES, fixups, symbols)."""
    pytest.importorskip("iced_x86")
    obj = Path(__file__).resolve().parents[2] / "tests/fixtures/bench/fpbench-v-g3.obj"
    done = run("x86_16-wat", obj)
    assert done.returncode == 0 and done.stdout.split() == ["1135", "333", "3"], done.stderr
    assert obj.stat().st_size > 4000


def test_an_elf_object_counts_text_and_data_sections(tmp_path, needs_binaries):
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


def test_a_missing_or_empty_object_is_an_error(tmp_path, needs_binaries):
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


def test_an_x86_64_object_with_unwind_tables_counts_no_eh_frame_in_code_or_data(tmp_path):
    """An A/B read `size`'s text+data column: recpow x86_64 -Os showed +4 B where the code was -28 B, the 32 B being the `.eh_frame` of one more function (main
    inlined it). Two functions with unwind tables: CODE + ALIGN is the `.text` and DATA leaves `.eh_frame` out."""
    c = tmp_path / "u.c"
    c.write_text("int g(int n) { return n * 3 + 1; }\n__attribute__((noinline)) int f(int n) { return g(n) + g(n + 1); }\n")
    obj = tmp_path / "u.o"
    subprocess.run(["gcc", "-Os", "-fno-pie", "-fasynchronous-unwind-tables", "-fno-inline", "-c", str(c), "-o", str(obj)], check=True)
    sections = {row.split()[0]: int(row.split()[1]) for row in subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True).stdout.splitlines() if len(row.split()) > 2 and row.split()[1].isdigit()}
    code, data, align = (int(n) for n in run("x86_64-linux", obj).stdout.split())
    assert sections[".eh_frame"] > 0, "premise: the object carries an .eh_frame"
    naive = int(subprocess.run(["size", str(obj)], capture_output=True, text=True).stdout.splitlines()[1].split()[0])
    assert naive > code + align, "premise: size's text column counts the .eh_frame the code column must not"
    assert code + align == sections[".text"], (code, align, sections)
    assert data == 0, (code, data, align, sections)


def test_object_bytes_reads_an_elf_without_importing_the_compiler_tooling(tmp_path):
    """`import sizes` pulls llrmbin, benchlist and run_tests, which look for compiler binaries: an ELF size must not need them (m64's review of #8533)."""
    source = tmp_path / "a.s"
    source.write_text(".text\nf: ret\n")
    obj = tmp_path / "a.o"
    subprocess.run(["as", "--32", str(source), "-o", str(obj)], check=True)
    probe = "import runpy, sys; sys.argv = ['o', 'x86_32-linux', %r]; sys.path.insert(0, %r)\ntry: runpy.run_path(%r, run_name='__main__')\nexcept SystemExit: pass\nprint('sizes' in sys.modules, 'llrmbin' in sys.modules)" % (str(obj), str(TOOL.parent), str(TOOL))
    done = subprocess.run([sys.executable, "-c", probe], capture_output=True, text=True)
    assert done.stdout.split()[-2:] == ["False", "False"], done.stdout + done.stderr


def test_a_65c02_object_gives_its_code_not_an_unreadable_error(tmp_path):
    """cells.sh listed every 65c02 cell EMPTY (`no reader`, then ca65 refusing `stz` without --cpu 65C02) beside 6502 cells that read."""
    source = tmp_path / "o.s"
    source.write_text("\t.segment \"CODE\"\n_f:\n\tstz $10\n\trts\n")
    done = run("65c02-sim65", source)
    assert done.returncode == 0 and done.stdout.split() == ["3", "0", "0"], done.stderr


def test_a_6502_object_still_refuses_a_65c02_instruction(tmp_path):
    source = tmp_path / "o.s"
    source.write_text("\t.segment \"CODE\"\n_f:\n\tstz $10\n\trts\n")
    done = run("6502-sim65", source)
    assert done.returncode == 2 and done.stdout == "", done.stdout


def test_a_wasm_object_counts_its_code_and_data_sections(tmp_path):
    """cells.sh listed every wasm cell EMPTY: wasm was refused outright. A module with one empty function: Code section 4 bytes, no Data."""
    module = bytes.fromhex("0061736d01000000" "010401600000" "03020100" "0a040102000b")
    obj = tmp_path / "o.wasm"
    obj.write_bytes(module)
    done = run("wasm32", obj)
    assert done.returncode == 0 and done.stdout.split() == ["4", "0", "0"], done.stderr


def test_a_32_bit_omf_object_is_read_by_the_omf_reader_not_refused(tmp_path, monkeypatch):
    """x86_32-wat objects are OMF too: `footprint` sent only x86_16 to the OMF reader and answered x86_32 with "no reader" (the known_loads_walks byte column,
    2026-11-10). The reader is stood in for here: the routing is under test, not the decoder."""
    import types

    obj = tmp_path / "o.obj"
    obj.write_bytes(b"\x80" + bytes(15))  # a THEADR record's first byte: not ELF
    fake = types.ModuleType("sizes")
    fake.code_size = lambda path: types.SimpleNamespace(code=40, align=3)
    fake.data_bytes = lambda path: (5, 2)
    monkeypatch.setitem(sys.modules, "sizes", fake)
    monkeypatch.syspath_prepend(str(TOOL.parent))
    import object_bytes

    found = object_bytes.footprint("x86_32-wat", obj)
    assert (found.code, found.data, found.bss, found.align) == (40, 5, 2, 3)


def test_a_stale_binaries_refusal_exits_non_zero_and_prints_no_byte_count(tmp_path, monkeypatch, capsys):
    """`sizes` refuses a tree whose release binaries are older than their source (SystemExit with the reason). The reader must pass that on: no number on stdout,
    a non-zero exit. A wrapper that took the last output line as the count printed the reason as the bytes (known_loads_walks, 2026-11-10)."""
    import types

    obj = tmp_path / "o.obj"
    obj.write_bytes(b"\x80" + bytes(15))

    def refuse(path):
        raise SystemExit("stale binaries in target/release: llrm-pass older than the source they are built from")

    fake = types.ModuleType("sizes")
    fake.code_size = refuse
    fake.data_bytes = refuse
    monkeypatch.setitem(sys.modules, "sizes", fake)
    monkeypatch.syspath_prepend(str(TOOL.parent))
    import object_bytes

    with pytest.raises(SystemExit) as refused:
        object_bytes.main(["object_bytes.py", "x86_32-wat", str(obj)])
    assert "stale binaries" in str(refused.value) and refused.value.code != 0
    assert capsys.readouterr().out == ""
