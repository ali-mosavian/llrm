"""compile-gap measures a compile that writes an object, as the compilers it is compared with do."""
from __future__ import annotations

from pathlib import Path

SOURCE = (Path(__file__).resolve().parent / "compile-gap.py").read_text()


def test_the_compile_gap_writes_an_object_and_not_assembly_to_dev_null():
    """It printed assembly to /dev/null: llrm's text printer (21 M of d_faces -O2) stood for its object write (22 M), and
    clang and gcc did not encode or assemble anything."""
    assert "/dev/null" not in SOURCE
    assert '"-S"' not in SOURCE
    assert '"-c"' in SOURCE
