#!/usr/bin/env python3
"""Same-width float sources for the SDCC comparison: `same_width.py SRC_DIR OUT_DIR [NAME ...]`.

SDCC's `double` is 32 bits, ours is IEEE f64. A source built with `#define double float` alone still has C `double`
literals (`2.0`), so `y * 2.0` promotes to f64 and links the f64 library: float_return read +2530 B against SDCC for
that reason, not for any library gap. Here every floating literal gets an `f` suffix too, and the output is refused
if one is left. Feed OUT_DIR to `sweep.py --src`: it refuses a cell whose assembly still calls an f64 helper.
"""
import re, sys
from pathlib import Path

# comments and string/char literals are copied untouched; the rest is code
SPAN = re.compile(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'', re.S)
# a decimal floating literal: a point or an exponent, no suffix yet; hex literals (0x1e3) are not matched
FLOAT = re.compile(r"(?<![\w.])(?!0[xX])(\d+\.\d*|\.\d+|\d+(?=[eE][+-]?\d))(?:[eE][+-]?\d+)?(?![\w.])")


def code_spans(text):
    """(is_code, piece) pairs covering `text`."""
    at = 0
    for m in SPAN.finditer(text):
        yield True, text[at:m.start()]
        yield False, m.group(0)
        at = m.end()
    yield True, text[at:]


def double_literals(text):
    """The floating literals of `text`'s code that carry no `f` suffix."""
    return [m.group(0) for is_code, piece in code_spans(text) if is_code for m in FLOAT.finditer(piece)]


def f64_helpers(asm):
    """The soft-float f64 helpers (`__muldf3`, `__extendsfdf2`, ...) `asm` calls: a same-width program must call none."""
    return sorted(set(re.findall(r"call\s+(__\w*df\w*)", asm)))


def same_width(text):
    """`text` with `double` as `float` and every floating literal single precision; `ValueError` if one is left."""
    body = "".join(FLOAT.sub(lambda m: m.group(0) + "f", piece) if is_code else piece for is_code, piece in code_spans(text))
    out = "#define double float\n" + body
    if left := double_literals(out):
        raise ValueError(f"double literals left: {left[:3]}")
    return out


def prepare(sources, out):
    """`sources` with every one that names `double` replaced by its same-width copy in `out` (with its `.out`); the rest as they are."""
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    made = []
    for path in sources:
        text = path.read_text()
        if not re.search(r"\bdouble\b", text):
            made.append(path)
            continue
        (out / path.name).write_text(same_width(text))
        expected = path.with_suffix(".out")
        if expected.exists():
            (out / expected.name).write_text(expected.read_text())
        made.append(out / path.name)
    return made


def main():
    src, out, names = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3:]
    out.mkdir(parents=True, exist_ok=True)
    for path in sorted(src.glob("*.c")):
        if names and path.stem not in names:
            continue
        (out / path.name).write_text(same_width(path.read_text()))
        expected = path.with_suffix(".out")
        if expected.exists():
            (out / expected.name).write_text(expected.read_text())


if __name__ == "__main__":
    main()
