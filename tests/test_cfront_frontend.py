"""Borland C that Open Watcom's front end refused, through wccq itself.

Skipped where owshim/build.sh has not built wccq or Open Watcom's headers
are not installed.
"""

from pathlib import Path

import pytest

from qbopt.cfront import compile as cfront

WATCOM_H = Path.home() / "dos" / "WATCOM" / "h"
pytestmark = pytest.mark.skipif(
    not cfront.WCCQ.exists() or not WATCOM_H.is_dir(), reason="needs owshim/bin/wccq and Open Watcom's headers"
)


def _stream(tmp_path: Path, source: str) -> str:
    unit = tmp_path / "unit.c"
    unit.write_text(source)
    return cfront.recorded(unit, [str(WATCOM_H)])


def test_borland_alloc_h_is_found(tmp_path):
    """sc.c includes <alloc.h>, which Open Watcom calls malloc.h."""
    assert "CGProcDecl" in _stream(tmp_path, "#include <alloc.h>\nvoid *get( void ) { return malloc( 4 ); }\n")


def test_inline_assembly_takes_387_instructions(tmp_path):
    """d_poly.c's `fsin` in __asm: E1156 invalid instruction with current CPU setting."""
    source = "double s( double r ) { double x; __asm { fld r\n fsin\n fstp x } return x; }\n"
    assert "CGProcDecl" in _stream(tmp_path, source)
