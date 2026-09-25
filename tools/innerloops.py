"""
Every innermost call-free loop in an OMF object, as the bytes decode.

The scoreboard for loop quality: instructions and memory operands per loop,
read from the object, never from a dump taken before encoding.

    uv run python tools/innerloops.py A.OBJ [--names DUMPDIR] [--show]
    uv run python tools/innerloops.py A.OBJ B.OBJ --names DUMPDIR [--show]

Loops are named `FUNCTION#k`, k counting a function's loops in address order.
Names come from the object's public symbols; a QB object publishes only its
entry, so `--names` takes the `--dump` directory, whose files are numbered in
emission order.
"""

import re
import sys
import bisect
import argparse
from pathlib import Path
from dataclasses import dataclass

import iced_x86 as ix


@dataclass
class Loop:
    name: str
    at: int
    lines: list[str]
    memory: int

    @property
    def size(self) -> int:
        return len(self.lines)


def _index(body: bytes, at: int) -> tuple[int, int]:
    if body[at] & 0x80:
        return ((body[at] & 0x7F) << 8) | body[at + 1], at + 2
    return body[at], at + 1


def records(data: bytes):
    at = 0
    while at < len(data):
        kind = data[at]
        size = int.from_bytes(data[at + 1 : at + 3], "little")
        yield kind, data[at + 3 : at + 3 + size - 1]
        at += 3 + size


def image(data: bytes) -> tuple[bytes, dict[int, str]]:
    """The code segment's bytes, and the public names in it by offset."""
    lnames, segments, chunks, publics = [""], [], {}, {}
    for kind, body in records(data):
        if kind == 0x96:
            at = 0
            while at < len(body):
                lnames.append(body[at + 1 : at + 1 + body[at]].decode("latin-1"))
                at += 1 + body[at]
        elif kind in (0x98, 0x99):
            at = 1 + (3 if body[0] >> 5 == 0 else 0) + (4 if kind == 0x99 else 2)
            name, at = _index(body, at)
            klass, at = _index(body, at)
            segments.append(lnames[klass])
        elif kind in (0xA0, 0xA1):
            segment, at = _index(body, 0)
            width = 4 if kind == 0xA1 else 2
            offset = int.from_bytes(body[at : at + width], "little")
            for index, byte in enumerate(body[at + width :]):
                chunks.setdefault(segment, {})[offset + index] = byte
        elif kind in (0x90, 0x91, 0xB6, 0xB7):
            _group, at = _index(body, 0)
            segment, at = _index(body, at)
            at += 2 if segment == 0 else 0
            width = 4 if kind & 1 else 2
            while at < len(body):
                name = body[at + 1 : at + 1 + body[at]].decode("latin-1")
                at += 1 + body[at]
                publics.setdefault(segment, {})[int.from_bytes(body[at : at + width], "little")] = name
                at += width
                _type, at = _index(body, at)
    code = next(index + 1 for index, klass in enumerate(segments) if klass.endswith("CODE"))
    bytes_ = chunks.get(code, {})
    return bytes(bytes_.get(at, 0) for at in range(max(bytes_, default=-1) + 1)), publics.get(code, {})


def _dumped(directory: Path) -> list[str]:
    names = {}
    for one in directory.iterdir():
        match = re.match(r"(\d+)-(.+?)-\d+-", one.name)
        if match:
            names[int(match.group(1))] = match.group(2)
    return [names[key] for key in sorted(names)]


def _entered(text: list[ix.Instruction], formatter: ix.Formatter) -> list[int]:
    """Where a BC-framed procedure starts: `cx` and `bx` set, then the far frame call."""
    starts = []
    for at in range(len(text) - 2):
        first, second = formatter.format(text[at]), formatter.format(text[at + 1])
        if (
            (first.startswith("mov cx,") or first == "xor cx,cx")
            and (second.startswith("mov bx,") or second == "xor bx,bx")
            and text[at + 2].flow_control == ix.FlowControl.CALL
        ):
            starts.append(text[at].ip)
    return starts


def loops(data: bytes, names: list[str] | None = None) -> list[Loop]:
    code, publics = image(data)
    formatter = ix.Formatter(ix.FormatterSyntax.MASM)
    text = list(ix.Decoder(16, code, ip=0))
    if names:
        starts = _entered(text, formatter)
        starts = ([0] if len(starts) < len(names) else []) + starts
        named = dict(zip(starts, names))
    else:
        named = publics
    offsets = sorted(named)
    where = {one.ip: index for index, one in enumerate(text)}
    edges = [
        (where[one.near_branch_target], index)
        for index, one in enumerate(text)
        if one.flow_control in (ix.FlowControl.CONDITIONAL_BRANCH, ix.FlowControl.UNCONDITIONAL_BRANCH)
        and one.near_branch_target <= one.ip
        and one.near_branch_target in where
    ]
    inner = sorted(
        edge for edge in edges if not any(other != edge and edge[0] <= other[0] and other[1] <= edge[1] for other in edges)
    )
    out, seen = [], {}
    for first, last in inner:
        body = text[first : last + 1]
        if any(one.flow_control in (ix.FlowControl.CALL, ix.FlowControl.INDIRECT_CALL) for one in body):
            continue
        owner = bisect.bisect_right(offsets, body[0].ip) - 1
        name = named[offsets[owner]] if owner >= 0 else "?"
        count = seen.get(name, 0)
        seen[name] = count + 1
        memory = sum(
            1
            for one in body
            for operand in range(one.op_count)
            if one.op_kind(operand) == ix.OpKind.MEMORY and one.mnemonic != ix.Mnemonic.LEA
        )
        lines = [f"{one.ip:04x}  {formatter.format(one)}" for one in body]
        out.append(Loop(f"{name}#{count}", body[0].ip, lines, memory))
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("objects", nargs="+", type=Path)
    parser.add_argument("--names", type=Path, help="a --dump directory naming the procedures")
    parser.add_argument("--show", action="store_true", help="print each loop's instructions")
    arguments = parser.parse_args()
    names = _dumped(arguments.names) if arguments.names else None
    runs = [{one.name: one for one in loops(path.read_bytes(), names)} for path in arguments.objects]
    for name in runs[0]:
        row = [run.get(name) for run in runs]
        sizes = "  ".join(f"{one.size:4d} {one.memory:3d}m" if one else "   -     " for one in row)
        mark = "  *" if len({(one.size, one.memory) if one else None for one in row}) > 1 else ""
        print(f"{name:<24} {sizes}{mark}")
        if arguments.show:
            for one in row:
                print("\n".join(f"    {line}" for line in one.lines) if one else "    -")
                print()


if __name__ == "__main__":
    main()
