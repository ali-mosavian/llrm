"""Two OMF objects compared as what they link to.

    uv run python tools/objcmp.py OURS.obj THEIRS.obj

Segment order, bytes, resolved fixups, group membership, external order and
public definitions are compared. Record order, LNAMES and index numbering are
not; what they mean is.
"""

import sys
from dataclasses import dataclass

from iced_x86 import Decoder
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt.objectfile import omf

FORMAT = Formatter(FormatterSyntax.MASM)


type FixupView = tuple[int, int, bool, str, int, int | None]
type SegmentView = tuple[int, bytes, list[FixupView]]


@dataclass(frozen=True, slots=True)
class View:
    segments: dict[str, SegmentView]
    groups: dict[str, list[str]]
    externals: list[str]
    publics: dict[str, tuple[int, int]]


def view(path: str) -> View:
    records = omf.read(path)
    segments = omf.segments(records)
    grouped = omf.groups(records)
    groups = dict(enumerate(grouped, 1))
    externals = omf.externals(records)

    def segment_name(index: int) -> str:
        if (segment := segments[index]) is None:
            raise ValueError(f"group member {index} is not a segment")
        return segment[0]

    members = {name: [segment_name(one) for one in indices] for name, indices in grouped.items()}

    def named(target: str, index: int) -> str:
        if target == "segment" and (segment := segments[index]) is not None:
            return segment[0]
        if target == "group":
            return groups[index]
        if target == "external":
            return externals[index]
        raise ValueError(f"unknown fixup target {target!r} at index {index}")

    out = {}
    for index, segment in enumerate(segments):
        if segment is None:
            continue
        name, size = segment
        fixups = sorted(
            (one.offset, one.loc, one.selfrel, named(one.target, one.index), one.disp, one.frame_method)
            for one in omf.fixups(records)
            if one.seg == index
        )
        out[name] = (size, omf.segment_image(records, index, size), fixups)
    # In order: LINK searches libraries for externals in the order it meets them.
    return View(out, members, externals[1:], omf.public_definitions(records))


def compared(ours: str, theirs: str, ignored_publics: frozenset[str] = frozenset()) -> list[str]:
    one, other = view(ours), view(theirs)
    problems = []
    if one.externals != other.externals:
        problems.append(f"externals {one.externals} != {other.externals}")
    if one.groups != other.groups:
        problems.append(f"groups {one.groups} != {other.groups}")
    publics = {name: where for name, where in one.publics.items() if name not in ignored_publics}
    other_publics = {name: where for name, where in other.publics.items() if name not in ignored_publics}
    if publics != other_publics:
        problems.append(f"publics {publics} != {other_publics}")
    if list(one.segments) != list(other.segments):
        problems.append(f"segments {list(one.segments)} != {list(other.segments)}")
    for name in one.segments.keys() & other.segments.keys():
        (size, image, fixups), (size2, image2, fixups2) = one.segments[name], other.segments[name]
        if size != size2:
            problems.append(f"{name}: {size:#x} bytes, not {size2:#x}")
        if name.endswith("_TEXT"):
            # Decoded: prefix order is the encoder's choice, `66 67` and `67 66` one instruction.
            mine, yours = _decoded(image), _decoded(image2)
            pairs = zip(mine, yours, strict=False)
            if differs := next((pair for pair in pairs if not _same(*pair)), None):
                problems.append(f"{name}: {differs[0]} vs {differs[1]}")
        else:
            at = next((n for n, (a, b) in enumerate(zip(image, image2, strict=False)) if a != b), None)
            if at is not None:
                problems.append(f"{name}: first byte differs at {at:#x}")
        extra, missing = sorted(set(fixups) - set(fixups2)), sorted(set(fixups2) - set(fixups))
        if extra or missing:
            problems.append(f"{name}: fixups only ours {extra[:4]}, only theirs {missing[:4]}")
    return problems


def _same(mine: tuple[str, int], yours: tuple[str, int]) -> bool:
    """The same instruction at the same length, wherever it sits."""
    return mine[0].split(" ", 1)[1] == yours[0].split(" ", 1)[1] and mine[1] == yours[1]


def _decoded(image: bytes) -> list[tuple[str, int]]:
    """Each instruction's text and length, a branch naming its target's instruction number.

    By number rather than address, so one length difference shows once and not
    again at every branch across it.
    """
    decoded = list(Decoder(16, image, ip=0))
    number = {one.ip: n for n, one in enumerate(decoded)}
    out = []
    for n, one in enumerate(decoded):
        text = FORMAT.format(one)
        if one.is_jcc_short_or_near or one.is_jmp_short_or_near or one.is_call_near:
            text = f"{text.split()[0]} #{number.get(one.near_branch_target, one.near_branch_target)}"
        out.append((f"{one.ip:04X} #{n} {text}", one.len))
    return out


def main() -> None:
    problems = compared(sys.argv[1], sys.argv[2])
    print("\n".join(problems) or "same")


if __name__ == "__main__":
    main()
