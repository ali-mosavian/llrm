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


def test_the_clang_compared_with_is_the_one_named_and_not_whichever_the_path_has():
    """The distro's clang links libLLVM as a shared library: 13 M instructions more to start than a static build, +20% on a
    compile of d_faces -O2 (554 M against 462 M). The column compares with the build named by `--clang`, and the distro's
    stays a second column."""
    assert '["clang"' not in SOURCE
    assert '"--clang"' in SOURCE and '"--distro-clang"' in SOURCE
