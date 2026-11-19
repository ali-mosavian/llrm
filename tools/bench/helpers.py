"""The library routines a reference program links, which its object's `code_bytes` leaves out.

A reference compiler may call a runtime helper (Turbo C's `LDIV`, Watcom's `__I4D`) where another inlines the operation. The helper's bytes are in the
program, in no object. `linked_helpers` finds the library modules that define what the program object leaves extern (the linker map names them),
once each, and sizes each module from its library (tools/sizes.py). Start-up modules, runtime services (a module that pulls in more library) and the C library
functions a program calls by name are not helpers.
"""

import re
import tempfile
from pathlib import Path

import sizes

STARTUP_SYMBOLS = {"_big_code_", "_cstart_"}  # Open Watcom's start-up pulls, not helpers the program calls


class HelperUnresolved(Exception):
    """An extern the map has no library module for: counting the program without it would understate it."""


def omf_library_module(library: Path, name: str, out: Path) -> Path:
    """The object module `name` of an OMF library (LIB header, pages of modules up to the dictionary), written to `out`."""
    data = library.read_bytes()
    page, dictionary = int.from_bytes(data[1:3], "little") + 3, int.from_bytes(data[3:7], "little")
    pos, module, title = page, bytearray(), ""
    while pos < dictionary:
        kind, length = data[pos], int.from_bytes(data[pos + 1:pos + 3], "little")
        module += data[pos:pos + 3 + length]
        if kind == 0x80:  # THEADR
            title = data[pos + 4:pos + 4 + data[pos + 3]].decode("latin-1")
        pos += 3 + length
        if kind in (0x8A, 0x8B):  # MODEND: the next module starts on a page boundary
            if title.upper() == name.upper():
                out.write_bytes(module)
                return out
            module, pos = bytearray(), -(-pos // page) * page
    raise KeyError(f"{name} is not a module of {library}")


def library_symbols(listing: Path) -> dict[str, tuple[Path, str] | None]:
    """{symbol: (library, module)} for the symbols the linker map lists under a `Module: LIB(NAME)` line; None for one under any other
    module (the corpus's support objects: `report`, the OS layer), which is not a helper."""
    symbols, current = {}, None
    for line in listing.read_text(errors="replace").splitlines():
        found = re.match(r"^Module: (.+\.[Ll][Ii][Bb])\(([\w.]+)\)", line)
        if found:
            current = (Path(found.group(1)), found.group(2))
        elif line.startswith("Module:"):
            current = False
        else:
            symbol = re.match(r"^[0-9a-f]{4}:[0-9a-f]{4}[*+]?\s+(\S+)", line)
            if symbol and current is not None:
                symbols[symbol.group(1)] = current or None
    return symbols


def written_identifiers(source: Path) -> set[str]:
    """Every identifier of a program's source and of the shared header it includes (bench_io.h maps `bench_open` to the C library's `open`)."""
    text = source.read_text(errors="replace")
    for header in (source.parent.parent / "bench_io.h", source.parent / "bench_io.h"):
        if header.exists():
            text += header.read_text(errors="replace")
    return set(re.findall(r"[A-Za-z_]\w*", text))


def written_by_the_program(name: str, written: set[str]) -> bool:
    """Whether the program's source names this extern: `_open` and `open_` are the C function `open`, `_report` is `report`. A routine the
    compiler introduced (`LMOD@`, `__I4D`: a long divide) is written nowhere."""
    return name.strip("_@") in written


def linked_helpers(obj: Path, listing: Path, written: set[str]) -> list[tuple[str, int]]:
    """[(module, code bytes)] of the compiler-support routines `obj` links: the library module of each extern the source `written` does not
    name (a C library function the program calls, `open` or `report`, is its own call, not a helper), once each. A support routine is self-contained
    (a long divide, a shift, a float-to-int convert); a module that pulls in other library modules is a runtime service every program of that
    compiler carries (floating-point initialisation), and is not counted. An extern the map does not
    define at all raises HelperUnresolved: the program would be counted without it."""
    symbols = library_symbols(listing)
    found, seen = [], set()
    with tempfile.TemporaryDirectory(prefix="helpers-") as scratch:
        for name in sizes.externs(obj):
            if name in STARTUP_SYMBOLS or written_by_the_program(name, written):
                continue
            if name not in symbols:
                raise HelperUnresolved(name)
            if symbols[name] is None or symbols[name] in seen:
                continue
            seen.add(symbols[name])
            library, module = symbols[name]
            part = omf_library_module(library, module, Path(scratch) / f"{len(seen)}.obj")
            if any(symbols.get(one) for one in sizes.externs(part)):
                continue  # it pulls in more library (the floating-point set-up, the string-to-double reader): a runtime service, not a routine
            found.append((module, sizes.code_bytes(part)))
    return found
