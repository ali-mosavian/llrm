"""Borland C that Open Watcom's front end refused, through wccq itself.

Skipped where owshim/build.sh has not built wccq or Borland's headers are
not installed.
"""

from pathlib import Path

import pytest

from qbopt.cfront import compile as cfront

BORLAND_H = Path.home() / "work" / "other" / "d32x" / "toolchains" / "bcpp31" / "INCLUDE"
pytestmark = pytest.mark.skipif(
    not cfront.WCCQ.exists() or not BORLAND_H.is_dir(), reason="needs owshim/bin/wccq and Borland's headers"
)


def _stream(tmp_path: Path, source: str) -> str:
    unit = tmp_path / "unit.c"
    unit.write_text(source)
    return cfront.recorded(unit, [str(BORLAND_H)])


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


def test_borland_names_a_parameter_segment(tmp_path):
    """Borland's dos.h declares `peek( unsigned __segment, ... )`; `__segment`
    is an Open Watcom keyword, and mdl_ai.c stopped with E1060 Invalid type."""
    assert "CGProcDecl" in _stream(
        tmp_path, "int peek( unsigned __segment, unsigned __offset );\nint get( void ) { return peek( 0, 0x46c ); }\n"
    )


def test_stdc_is_undefined_as_in_bcc(tmp_path):
    """Open Watcom defines __STDC__ 1, which hid dos.h's FP_OFF macro: mdl_ai.c
    called an undefined `_FP_OFF` and QCPORT.EXE did not link."""
    stream = _stream(tmp_path, "#include <dos.h>\nunsigned off( void far *p ) { return FP_OFF( p ); }\n")
    assert "CGProcDecl" in stream and "FP_OFF" not in stream
