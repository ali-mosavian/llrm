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


def test_main_keeps_the_default_convention(tmp_path):
    """Open Watcom turns main into __watcall, `main_` with register parameters;
    Borland's c0 calls `_main` cdecl."""
    stream = _stream(tmp_path, "int main( void ) { return 0; }\n")
    symbol = next(line for line in stream.splitlines() if 'name="main"' in line)
    convention = stream.splitlines()[stream.splitlines().index(symbol) + 1]
    assert 'pattern="_*"' in symbol and "class=0x80" in convention and "parms=[]" in convention


def test_pointers_to_one_integer_size_convert(tmp_path):
    """input.c passes a `short *` for an `int *`: Borland warns, Open Watcom stopped with E1176."""
    source = "static short get( int *p ) { return *p; }\nshort use( short *k ) { return get( k ); }\n"
    assert "CGProcDecl" in _stream(tmp_path, source)


def test_inline_assembly_takes_387_instructions(tmp_path):
    """d_poly.c's `fsin` in __asm: E1156 invalid instruction with current CPU setting."""
    source = "double s( double r ) { double x; __asm { fld r\n fsin\n fstp x } return x; }\n"
    assert "CGProcDecl" in _stream(tmp_path, source)


def test_relative_source_and_include(tmp_path, monkeypatch):
    """wccq runs in its scratch directory, where `fixtures/c/x.c` and `-I src`
    no longer resolved: E1051 unable to open."""
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "one.h").write_text("int one( void );\n")
    (tmp_path / "unit.c").write_text('#include "one.h"\nint two( void ) { return one() + 1; }\n')
    monkeypatch.chdir(tmp_path)
    assert "CGProcDecl" in cfront.recorded(Path("unit.c"), ["src"])
