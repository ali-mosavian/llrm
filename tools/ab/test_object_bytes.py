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
    assert done.returncode == 0 and done.stdout.split() == ["1", "0"], done.stderr


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


def test_an_omf_object_counts_code_and_data_not_the_file(tmp_path, needs_binaries):
    """The m16 fixture: 4327 file bytes hold 1135 of code and 333 of data (the rest are THEADR, LNAMES, fixups, symbols)."""
    pytest.importorskip("iced_x86")
    obj = Path(__file__).resolve().parents[2] / "tests/fixtures/bench/fpbench-v-g3.obj"
    done = run("x86_16-wat", obj)
    assert done.returncode == 0 and done.stdout.split() == ["1135", "333"], done.stderr
    assert obj.stat().st_size > 4000


def test_an_elf_object_counts_text_and_data_sections(tmp_path, needs_binaries):
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


def test_a_missing_or_empty_object_is_an_error(tmp_path, needs_binaries):
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
