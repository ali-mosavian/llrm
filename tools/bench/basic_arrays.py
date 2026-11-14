"""A BASIC program's module-level arrays and where each compiler keeps them.

A BASIC reference allocates some arrays at run time (the footprint of its exe does not hold them) and holds others in its data. `static` = data + bss
counts only the held ones: an array held by one side and allocated by the other made the first look bigger. `run_time_bytes` is what a side allocates
at run time, for the judge to add to its data + bss.

What each side keeps in its image (read from the maps, per program: tools/bench/test_basic_arrays.py states the cases):
- Microsoft's BC (QB 4.5, PDS 7.1, VB/DOS): a `DIM` of module level whose bounds are each a numeric literal or one bare CONST name; a bound that is
  an expression (`N - 1`), `$DYNAMIC` and `REDIM` allocate at run time.
- llrm: everything but `$DYNAMIC` and `REDIM`.
Arrays inside a SUB or FUNCTION are on the stack for every side and are not static.
"""

import re
from dataclasses import dataclass
from pathlib import Path

SIZES = {"INTEGER": 2, "LONG": 4, "SINGLE": 4, "DOUBLE": 8, "STRING": 4}  # a string array holds one 4-byte descriptor per element
SUFFIXES = {"%": "INTEGER", "&": "LONG", "!": "SINGLE", "#": "DOUBLE", "$": "STRING"}
DEFTYPES = {"DEFINT": "INTEGER", "DEFLNG": "LONG", "DEFSNG": "SINGLE", "DEFDBL": "DOUBLE", "DEFSTR": "STRING"}
LITERAL = re.compile(r"\s*-?\d+\s*")
NAME = re.compile(r"\s*[A-Za-z_]\w*\s*")


@dataclass(frozen=True)
class Array:
    name: str
    bytes: int
    simple_bounds: bool  # every bound a numeric literal or one bare CONST name
    run_time: bool  # REDIM, or `$DYNAMIC` in force


def _evaluate(text: str, consts: dict[str, int]) -> int:
    for name, value in consts.items():
        text = re.sub(rf"\b{name}\b", str(value), text, flags=re.I)
    return int(eval(re.sub(r"[%&!#]", "", text).replace("\\", "//"), {"__builtins__": {}}))  # noqa: S307: digits and operators of a CONST expression


def _default_type(first: str, defaults: list[tuple[str, str, str]]) -> str:
    for kind, low, high in reversed(defaults):
        if low <= first.upper() <= high:
            return kind
    return "SINGLE"


def arrays(text: str) -> list[Array]:
    """The module-level arrays of the program, in source order. A bound or element type it cannot size raises ValueError: a guess would be a measurement."""
    consts: dict[str, int] = {}
    defaults: list[tuple[str, str, str]] = []
    found: list[Array] = []
    dynamic, depth = False, 0
    for raw in text.replace("\r", "").split("\n"):
        meta = re.match(r"\s*(?:'|REM\b)\s*\$(DYNAMIC|STATIC)", raw, re.I)
        if meta:
            dynamic = meta.group(1).upper() == "DYNAMIC"
            continue
        code = raw.split("'")[0].strip()
        if re.match(r"(SUB|FUNCTION)\b", code, re.I) and not re.match(r"(SUB|FUNCTION)\b.*\bEND\b", code, re.I):
            depth += 1
            continue
        if re.match(r"END\s+(SUB|FUNCTION)\b", code, re.I):
            depth -= 1
            continue
        if depth:
            continue
        for kind, ranges in re.findall(r"(?i)\b(DEFINT|DEFLNG|DEFSNG|DEFDBL|DEFSTR)\s+([A-Z](?:\s*-\s*[A-Z])?(?:\s*,\s*[A-Z](?:\s*-\s*[A-Z])?)*)", code):
            for one in ranges.split(","):
                low, _, high = one.strip().partition("-")
                defaults.append((DEFTYPES[kind.upper()], low.strip().upper(), (high or low).strip().upper()))
        const = re.match(r"CONST\s+(.*)$", code, re.I)
        if const:
            for part in re.split(r",\s*(?=\w+\s*=)", const.group(1)):
                name, _, value = part.partition("=")
                consts[name.strip().rstrip("%&!#$")] = _evaluate(value, consts)
            continue
        declared = re.match(r"(DIM(?:\s+SHARED)?|REDIM(?:\s+SHARED)?)\s+(.*)$", code, re.I)
        if not declared:
            continue
        for part in re.split(r",\s*(?=\w+[%&!#$]?\s*(?:\(|AS\b|$))", declared.group(2)):
            one = re.match(r"(\w+)([%&!#$]?)\s*\((.*)\)\s*(?:AS\s+(\w+))?\s*$", part.strip(), re.I)
            if not one:
                continue
            name, suffix, bounds, kind = one.groups()
            count, simple = 1, True
            for dimension in bounds.split(","):
                low, high = re.split(r"\bTO\b", dimension, flags=re.I) if re.search(r"\bTO\b", dimension, re.I) else ("0", dimension)
                simple &= all(LITERAL.fullmatch(side) or (NAME.fullmatch(side) and side.strip() in consts) for side in (low, high))
                count *= _evaluate(high, consts) - _evaluate(low, consts) + 1
            kind = (kind or SUFFIXES.get(suffix) or _default_type(name[0], defaults)).upper()
            if kind not in SIZES:
                raise ValueError(f"{name}: element type {kind} has no size here")
            found.append(Array(name, count * SIZES[kind], simple, dynamic or declared.group(1).upper().startswith("REDIM")))
    return found


def run_time_bytes(source: Path, side: str) -> int:
    """What `side` ('llrm' or 'reference') allocates at run time for the program's module-level arrays, beyond the image's data + bss."""
    found = arrays(source.read_text(errors="replace"))
    if side == "llrm":
        return sum(one.bytes for one in found if one.run_time)
    if side != "reference":
        raise ValueError(f"side {side!r}: llrm or reference")
    return sum(one.bytes for one in found if one.run_time or not one.simple_bounds)
