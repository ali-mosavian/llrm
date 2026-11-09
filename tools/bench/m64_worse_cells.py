#!/usr/bin/env python3
"""m64_worse_cells.py MATRIX.tsv: the cells where ours is bigger than the best of gcc and clang, from a matrix64 table, on two axes:
`code` (padding-free executable bytes) and `code+data` (plus the object's data: tools/ab/object_bytes.py, .data, .rodata, .bss and the
small-data forms; an earlier column left .bss out and undercounted zero-filled data, grep 81 B against 3137 B).

    m64_worse_cells.py MATRIX.tsv          the table
    m64_worse_cells.py --data OBJECT       the DATA bytes of one object, as the matrix prints them

A table whose header does not say `data=object_bytes` was built with that older column and is refused. So is one that does not say
`code_bytes padding-free`: code is judged without alignment padding in every column, and padding is reported as its own column (`pad`,
emitted text less code). A column whose code exceeds its emitted text, or that has no emitted text, was measured another way (padding on
one side only) and is refused like a missing number.

A cell with no number for ours, gcc or clang is refused (exit 2, listed), never compared against the next-best column: a missing
reference understated the z80 gaps by up to 1.7 kB."""
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ab"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

DATA_MARK = "data=object_bytes"
PAD_MARK = "code_bytes padding-free"


def object_data(obj: Path) -> int:
    """The DATA bytes of one x86-64 object: the one reader (tools/ab/object_bytes.py), never a second implementation."""
    import object_bytes

    return object_bytes.object_bytes("x86_64-linux", obj)[1]

COLUMNS = ("ours", "gcc", "clang")


def cells(lines: list[str]) -> dict[tuple[str, str], dict[str, tuple[str, str, str]]]:
    """(program, level) -> compiler -> (code_bytes, data_bytes, emitted_text_bytes) as text."""
    found: dict[tuple[str, str], dict[str, tuple[str, str, str]]] = defaultdict(dict)
    for line in lines:
        f = line.rstrip("\n").split("\t")
        if len(f) >= 7 and not line.startswith(("#", "target")) and f[3] in COLUMNS:
            found[(f[1], f[2])][f[3]] = (f[4], f[6], f[5])
    return found


def padding(table: dict[tuple[str, str], dict[str, tuple[str, str, str]]]) -> dict[tuple[str, str], dict[str, int]]:
    """(program, level) -> compiler -> alignment padding bytes (emitted text less padding-free code); only for cells `worse` accepts."""
    return {key: {c: int(row[c][2]) - int(row[c][0]) for c in COLUMNS} for key, row in table.items() if not unusable(row)}


def unusable(row: dict[str, tuple[str, str, str]]) -> list[str]:
    """Why a cell cannot be compared: a missing code, data or emitted-text number, or code above the emitted text (not padding-free)."""
    why = []
    for c in COLUMNS:
        parts = row.get(c, ("", "", ""))
        if not all(part.isdigit() for part in parts[:2]):
            why.append(f"no {c}")
        elif not parts[2].isdigit():
            why.append(f"{c} has no emitted text, its padding cannot be told")
        elif int(parts[0]) > int(parts[2]):
            why.append(f"{c} code {parts[0]} above its emitted text {parts[2]}: not padding-free, padding on one side only")
    return why


AXES = {"code": lambda code, data: code, "code+data": lambda code, data: code + data}


def worse(table: dict[tuple[str, str], dict[str, tuple[str, str, str]]]) -> tuple[list[tuple[str, str, str, int, int]], list[str]]:
    """Rows (axis, program, level, ours, best); a missing code or data number is refused, never read as zero."""
    missing, rows = [], []
    for (program, level), row in sorted(table.items()):
        gone = unusable(row)
        if gone:
            missing.append(f"{program} {level}: {', '.join(gone)}")
            continue
        for axis, size in AXES.items():
            value = {c: size(int(row[c][0]), int(row[c][1])) for c in COLUMNS}
            best = min(value["gcc"], value["clang"])
            if value["ours"] > best:
                rows.append((axis, program, level, value["ours"], best))
    return rows, missing


if __name__ == "__main__":
    if sys.argv[1] == "--data":
        print(object_data(Path(sys.argv[2])))
        sys.exit(0)
    text = open(sys.argv[1]).read()
    if DATA_MARK not in text.splitlines()[0]:
        sys.exit(f"{sys.argv[1]}: the header does not say {DATA_MARK}: its data column left .bss out; rebuild the table")
    if PAD_MARK not in text.splitlines()[0]:
        sys.exit(f"{sys.argv[1]}: the header does not say {PAD_MARK}: its code column may carry alignment padding on one side only; rebuild the table")
    table = cells(text.splitlines())
    rows, missing = worse(table)
    pad = padding(table)
    for axis, program, level, ours, best in sorted(rows, key=lambda r: (r[0], -(r[3] - r[4]) / max(r[4], 1))):
        print(f"{axis}\t{program}\t{level}\t{ours}\t{best}\t{100 * (ours - best) / max(best, 1):+.1f}%\tpad {pad[(program, level)]['ours']}/{pad[(program, level)]['gcc']}/{pad[(program, level)]['clang']}")
    for line in missing:
        print("MISSING", line, file=sys.stderr)
    sys.exit(2 if missing else 0)
