"""tools/debug-matrix.py reads what an object carries: OMF DWARF was reported "not emitted" (rows for C, Nib and QB), as
only CodeView and Turbo Debugger were looked for in an OMF object."""

import importlib.util
import subprocess
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("debug_matrix", TOOLS / "debug-matrix.py")
matrix = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matrix)


def test_an_omf_object_with_dwarf_is_found_to_carry_it(tmp_path):
    out = tmp_path / "k.obj"
    program = matrix.ROOT / "tests/fixtures/matrix/known.nib"
    nib = matrix.llrmbin.bin_dir() / "llrm-nib"
    subprocess.run([nib, "-m16", "-O0", matrix.flag("dwarf", "omf"), program, "-o", out], check=True, capture_output=True)
    assert matrix.present("omf", out) == {"dwarf"}


def test_open_watcoms_reader_gets_the_dwarf_it_reads():
    assert matrix.flag("dwarf", "omf") == "-gdwarf-2"
    assert matrix.flag("dwarf", "elf") == "-gdwarf"
