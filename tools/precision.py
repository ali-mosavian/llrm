"""The x87 precision a float row was compiled at, and the rule for comparing it with clang.

clang evaluates float expressions at their own type (SSE); llrm's default, `-fexcess-precision=fast`, keeps x87 intermediates at 80 bits, which is faster and
a different answer. A float row against clang is measured at `-fexcess-precision=standard`, and every row says which it was.
"""

from __future__ import annotations

import os
import re

PRECISIONS = ("fast", "standard")
ENV = "LLRM_EXCESS_PRECISION"
FLOAT = re.compile(r"\b(?:float|double)\b")


def current(value: str | None = None) -> str:
    """The precision named by `value`, else by LLRM_EXCESS_PRECISION, else the default `fast`."""
    value = value or os.environ.get(ENV) or "fast"
    if value not in PRECISIONS:
        raise SystemExit(f"precision: {value!r} is not one of {', '.join(PRECISIONS)}")
    return value


def flags(precision: str | None = None) -> list[str]:
    """The compiler flag for `precision`: none for the default."""
    return ["-fexcess-precision=standard"] if current(precision) == "standard" else []


def is_float(source: str) -> bool:
    """A row whose source declares float or double (comments aside)."""
    return bool(FLOAT.search(re.sub(r"//[^\n]*|/\*.*?\*/", "", source, flags=re.S)))


def label(precision: str | None = None) -> str:
    """The text every row prints."""
    return f"excess-precision={current(precision)}"


def require(row: str, source: str, precision: str | None, against_clang: bool, allow_fast: bool = False) -> None:
    """SystemExit when a float row would be compared with clang at the fast default, unless `allow_fast` names it."""
    if against_clang and is_float(source) and current(precision) != "standard" and not allow_fast:
        raise SystemExit(f"{row}: a float row against clang needs -fexcess-precision=standard (it ran at {current(precision)}): set {ENV}=standard, or name --allow-fast-float")
