#!/usr/bin/env python3
"""Compare the linked BASIC code footprint in two Microsoft LINK maps."""

import re
import argparse
from pathlib import Path

LINKED_CODE_CLASSES = frozenset({"BC_CODE", "CODE"})
LINK_ERROR = re.compile(r"(?:error L\d+|unresolved external|fatal error)", re.IGNORECASE)


class MapLinkError(ValueError):
    """A MAP records a failed link and is not a valid footprint input."""


def map_text(path: Path) -> str:
    text = path.read_text(encoding="latin1")
    match = LINK_ERROR.search(text)
    if match is not None:
        line_start = text.rfind("\n", 0, match.start()) + 1
        line_end = text.find("\n", match.end())
        if line_end < 0:
            line_end = len(text)
        raise MapLinkError(f"{path}: linker failure: {text[line_start:line_end].strip()}")
    return text


def code_segments(path: Path, segment_class: str = "BC_CODE") -> dict[str, int]:
    """Return linked segment lengths, excluding data and OMF bookkeeping."""
    segments: dict[str, int] = {}
    for line in map_text(path).splitlines():
        fields = line.split()
        if len(fields) < 5 or fields[4] != segment_class:
            continue
        name = fields[3]
        if name.endswith("_CODE"):
            name = name[:-5]
        size = fields[2]
        if not size.endswith("H"):
            continue
        segments[name] = int(size[:-1], 16)
    return segments


def segment_class_total(path: Path, classes: frozenset[str]) -> int:
    """Sum exact linked segment lengths for the requested MAP classes."""
    total = 0
    for line in map_text(path).splitlines():
        fields = line.split()
        if len(fields) < 5 or fields[4] not in classes:
            continue
        size = fields[2]
        if size.endswith("H"):
            total += int(size[:-1], 16)
    return total


def linked_code_totals(bc: Path, llrm: Path) -> tuple[int, int]:
    """Return whole-image executable code, including pulled runtime helpers."""
    return (
        segment_class_total(bc, LINKED_CODE_CLASSES),
        segment_class_total(llrm, LINKED_CODE_CLASSES),
    )


def compare_maps(bc: Path, llrm: Path) -> tuple[list[tuple[str, int, int]], tuple[int, int]]:
    baseline = code_segments(bc)
    candidate = code_segments(llrm)
    names = sorted(baseline.keys() | candidate.keys())
    rows = [(name, baseline.get(name, 0), candidate.get(name, 0)) for name in names]
    return rows, (sum(baseline.values()), sum(candidate.values()))


def signed(value: int) -> str:
    return f"{value:+,}"


def markdown(bc: Path, llrm: Path) -> str:
    rows, (bc_total, llrm_total) = compare_maps(bc, llrm)
    bc_linked, llrm_linked = linked_code_totals(bc, llrm)
    lines = [
        "| Module | BC bytes | llrm bytes | Delta |",
        "| --- | ---: | ---: | ---: |",
    ]
    for name, bc_size, llrm_size in rows:
        lines.append(f"| {name} | {bc_size:,} | {llrm_size:,} | {signed(llrm_size - bc_size)} |")
    lines.extend(
        [
            "| **BASIC-owned BC_CODE** | "
            f"**{bc_total:,}** | **{llrm_total:,}** | **{signed(llrm_total - bc_total)}** |",
            "| **Complete linked code** | "
            f"**{bc_linked:,}** | **{llrm_linked:,}** | **{signed(llrm_linked - bc_linked)}** |",
        ]
    )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bc", type=Path, help="MAP file from Microsoft BC output")
    parser.add_argument("llrm", type=Path, help="MAP file from llrm frontend output")
    args = parser.parse_args()
    try:
        report = markdown(args.bc, args.llrm)
    except MapLinkError as error:
        parser.error(str(error))
    print(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
