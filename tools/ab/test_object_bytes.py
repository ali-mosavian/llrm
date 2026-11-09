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
    assert done.returncode == 0 and done.stdout.split() == ["4", "0"], done.stderr
    assert source.stat().st_size > 1000


def test_a_z80_object_counts_its_data_areas_apart(tmp_path):
    source = tmp_path / "o.s"
    source.write_text("\t.area _CODE\n_f::\n\tret\n\t.area _CONST\n_t::\n\t.db 1,2,3,4,5\n")
    done = run("z80-sdcc", source)
    assert done.stdout.split() == ["1", "5"], done.stderr


def test_an_omf_object_counts_code_and_data_not_the_file(tmp_path):
    """The m16 fixture: 4327 file bytes hold 1135 of code and 333 of data (the rest are THEADR, LNAMES, fixups, symbols)."""
    pytest.importorskip("iced_x86")
    obj = Path(__file__).resolve().parents[2] / "tests/fixtures/bench/fpbench-v-g3.obj"
    done = run("x86_16-wat", obj)
    assert done.returncode == 0 and done.stdout.split() == ["1135", "333"], done.stderr
    assert obj.stat().st_size > 4000


def test_an_elf_object_counts_text_and_data_sections(tmp_path):
    c = tmp_path / "o.c"
    c.write_text("int table[10] = {1}; const char k[7] = \"abcdef\"; int zero[20]; int f(void) { return table[1]; }\n")
    obj = tmp_path / "o.o"
    subprocess.run(["cc", "-O1", "-c", str(c), "-o", str(obj)], check=True)
    done = run("x86_32-linux", obj)
    code, data = (int(n) for n in done.stdout.split())
    assert code > 0 and data >= 40 + 7 + 80, done.stdout


def test_a_6502_object_counts_the_assembled_code_segment(tmp_path):
    source = tmp_path / "o.s"
    source.write_text("\t.segment \"CODE\"\n_f:\n\tlda #5\n\trts\n" + "; padding comment\n" * 40)
    done = run("6502-sim65", source)
    assert done.returncode == 0 and done.stdout.split() == ["3", "0"], done.stderr


def test_a_missing_or_empty_object_is_an_error(tmp_path):
    empty = tmp_path / "e.o"
    empty.write_bytes(b"")
    assert run("x86_32-linux", empty).returncode == 2
    assert run("x86_32-linux", tmp_path / "absent.o").returncode == 2


def test_a_65c02_object_gives_its_code_not_an_unreadable_error(tmp_path):
    """cells.sh listed every 65c02 cell EMPTY (`no reader`, then ca65 refusing `stz` without --cpu 65C02) beside 6502 cells that read."""
    source = tmp_path / "o.s"
    source.write_text("\t.segment \"CODE\"\n_f:\n\tstz $10\n\trts\n")
    done = run("65c02-sim65", source)
    assert done.returncode == 0 and done.stdout.split() == ["3", "0"], done.stderr


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
    assert done.returncode == 0 and done.stdout.split() == ["4", "0"], done.stderr
