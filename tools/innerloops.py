"""
Every innermost loop in an object, as the bytes decode.

The scoreboard for loop quality: instructions and memory operands per loop,
read from the object, never from a dump taken before encoding. OMF objects
(llrm, Open Watcom, BC) and ELF ones (gcc-ia16's x86, LLVM's msp430) alike.

    uv run python tools/innerloops.py A.OBJ [--names DUMPDIR] [--show] [--calls]
    uv run python tools/innerloops.py A.OBJ B.OBJ --names DUMPDIR [--show]

Loops are named `FUNCTION#k`, k counting a function's loops in address order.
Names come from the object's public symbols; a QB object publishes only its
entry, so `--names` takes the `--dump` directory, whose files are numbered in
emission order. A loop is its natural loop in the control-flow graph: the
blocks that reach its back edge without passing its header, wherever they
are laid out. Loops with a call are left out unless `--calls`.
"""

import re
import sys
import bisect
import argparse
from pathlib import Path
from dataclasses import dataclass, field

import iced_x86 as ix

sys.path.insert(0, str(Path(__file__).resolve().parent))

import msp430  # noqa: E402


@dataclass
class Loop:
    name: str
    at: int
    lines: list[str]
    memory: int
    calls: int = 0
    arch: str = "x86"
    body: list = field(default_factory=list, repr=False)
    header: int = 0
    latch: int = 0

    @property
    def size(self) -> int:
        return len(self.lines)


@dataclass
class Code:
    arch: str  # x86 or msp430
    data: bytes
    publics: dict[int, str]
    bits: int = 16  # x86's code width: 16 (real mode) or 32


# --- OMF ---------------------------------------------------------------------


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


def segments(data: bytes) -> list[tuple[bytes, dict[int, str], int]]:
    """Each code segment of an OMF object, in definition order: its bytes, the
    public names in it by offset, and its width in bits (SEGDEF's Use32 bit, or a 32-bit SEGDEF record)."""
    lnames, classes, chunks, publics, widths = [""], [], {}, {}, []
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
            classes.append(lnames[klass])
            widths.append(32 if kind == 0x99 or body[0] & 1 else 16)
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
    out = []
    for number, klass in enumerate(classes, 1):
        if klass.endswith("CODE"):
            bytes_ = chunks.get(number, {})
            code = bytes(bytes_.get(at, 0) for at in range(max(bytes_, default=-1) + 1))
            out.append((code, publics.get(number, {}), widths[number - 1]))
    return out


def image(data: bytes) -> tuple[bytes, dict[int, str]]:
    """An OMF object's first code segment's bytes, and the public names in it by offset."""
    code, publics, _ = segments(data)[0]
    return code, publics


# --- ELF ---------------------------------------------------------------------


def _elf(data: bytes, bits: int | None = None) -> list[Code]:
    """Each executable section of an ELF object, with its symbols. x86 ELF is 32-bit code unless `bits` says 16: the object does not say
    (gcc-ia16 writes 16-bit code in an ELF32 object of machine 386), and guessing from how the bytes decode picks the wrong width for
    code that decodes either way."""
    u16 = lambda at: int.from_bytes(data[at : at + 2], "little")  # noqa: E731
    u32 = lambda at: int.from_bytes(data[at : at + 4], "little")  # noqa: E731
    machine = u16(18)
    arch = {3: "x86", 105: "msp430"}.get(machine)
    if arch is None:
        raise ValueError(f"an ELF object for machine {machine}")
    shoff, shentsize, shnum = u32(32), u16(46), u16(48)
    sections = []
    for number in range(shnum):
        at = shoff + number * shentsize
        sections.append({"type": u32(at + 4), "flags": u32(at + 8), "offset": u32(at + 16), "size": u32(at + 20),
                         "link": u32(at + 24)})
    names: dict[int, dict[int, str]] = {}
    for section in sections:
        if section["type"] != 2:  # SHT_SYMTAB
            continue
        strings = sections[section["link"]]
        for at in range(section["offset"], section["offset"] + section["size"], 16):
            name_at, value, info, shndx = u32(at), u32(at + 4), data[at + 12], u16(at + 14)
            if info & 0xF in (1, 2) or (info & 0xF == 0 and info >> 4 == 1):  # object, function, global notype
                start = strings["offset"] + name_at
                name = data[start : data.index(b"\0", start)].decode("latin-1")
                if name:
                    names.setdefault(shndx, {})[value] = name
    return [
        Code(arch, data[s["offset"] : s["offset"] + s["size"]], names.get(number, {}), (bits or 32) if arch == "x86" else 16)
        for number, s in enumerate(sections)
        if s["type"] == 1 and s["flags"] & 4 and s["size"]  # PROGBITS, executable
    ]


def images(data: bytes, procedures: list[str] | None = None, bits: int | None = None) -> list[Code]:
    """The object's code. `procedures` names an object compiled one
    procedure per segment (llrm-nib --procedure-segments), in order."""
    if data[:4] == b"\x7fELF":
        return _elf(data, bits)
    found = segments(data)
    if procedures and len(procedures) == len(found):
        return [Code("x86", code, {0: name}, width) for (code, _, width), name in zip(found, procedures)]
    return [Code("x86", code, publics, width) for code, publics, width in found]


def listed(listing: Path) -> list[str]:
    """The procedures of an llrm listing, in order."""
    return re.findall(r"^(\S+) proc ", listing.read_text(), re.M)


# --- decoding ----------------------------------------------------------------


def _dumped(directory: Path) -> list[str]:
    names = {}
    for one in directory.iterdir():
        match = re.match(r"(\d+)-(.+?)-\d+-", one.name)
        if match:
            names[int(match.group(1))] = match.group(2)
    return [names[key] for key in sorted(names)]


def _entered(text: list, formatter: ix.Formatter) -> list[int]:
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


# A BASIC main module's code opens with the runtime's MODULE_CODE record,
# data of O_ENT bytes (runtime/inc/addr.inc) signed "bl"; its entry follows.
MODULE_CODE = 48

BRANCHES = (ix.FlowControl.CONDITIONAL_BRANCH, ix.FlowControl.UNCONDITIONAL_BRANCH)
ENDS = (
    ix.FlowControl.UNCONDITIONAL_BRANCH,
    ix.FlowControl.INDIRECT_BRANCH,
    ix.FlowControl.RETURN,
    ix.FlowControl.INTERRUPT,
    ix.FlowControl.EXCEPTION,
)
CALLS = (ix.FlowControl.CALL, ix.FlowControl.INDIRECT_CALL)


def _decode_one(arch: str, code: bytes, at: int, bits: int = 16):
    if arch == "msp430":
        return msp430.decode(code, at)
    one = ix.Decoder(bits, code[at:], ip=at).decode()
    return None if one.code == ix.Code.INVALID else one


def _target(one) -> int | None:
    if isinstance(one, msp430.Insn):
        return one.target
    if one.op0_kind in (ix.OpKind.NEAR_BRANCH16, ix.OpKind.NEAR_BRANCH32):
        return one.near_branch_target
    return None


def _decoded(code: bytes, entries: list[int], arch: str = "x86", bits: int = 16) -> list:
    """The instructions control reaches from `entries`, in address order.

    Following flow rather than sweeping keeps data in the code segment -- a
    module header, a jump table -- from being read as instructions and
    shifting the decode off every boundary after it."""
    found: dict[int, object] = {}
    pending = [one for one in entries if 0 <= one < len(code)]
    while pending:
        at = pending.pop()
        while 0 <= at < len(code) and at not in found:
            one = _decode_one(arch, code, at, bits)
            if one is None:
                break
            found[at] = one
            flow = one.flow_control
            if flow in (*BRANCHES, ix.FlowControl.CALL):
                target = _target(one)
                if target is not None:
                    pending.append(target)
            if flow in ENDS:
                break
            at = one.next_ip
    return [found[at] for at in sorted(found)]


def _successors(one, where: dict[int, int]) -> list[int]:
    out = []
    if one.flow_control not in ENDS and one.next_ip in where:
        out.append(where[one.next_ip])
    if one.flow_control in BRANCHES:
        target = _target(one)
        if target in where:
            out.append(where[target])
    return out


def dominators(text: list, entries: list[int]) -> list[int]:
    """Each instruction's immediate dominator (index), from a virtual root
    above `entries`; -1 for the root's children, -2 for the unreached.
    Cooper, Harvey and Kennedy's iteration over reverse postorder."""
    where = {one.ip: index for index, one in enumerate(text)}
    successors = [_successors(one, where) for one in text]
    order, seen = [], set()
    for entry in entries:
        if entry in seen:
            continue
        stack = [(entry, iter(successors[entry]))]
        seen.add(entry)
        while stack:
            node, rest = stack[-1]
            nxt = next((s for s in rest if s not in seen), None)
            if nxt is None:
                order.append(node)
                stack.pop()
            else:
                seen.add(nxt)
                stack.append((nxt, iter(successors[nxt])))
    order.reverse()
    rank = {node: at for at, node in enumerate(order)}
    predecessors: dict[int, list[int]] = {}
    for node in order:
        for s in successors[node]:
            predecessors.setdefault(s, []).append(node)
    ROOT = -1
    idom = {entry: ROOT for entry in entries if entry in rank}

    def intersect(a, b):
        while a != b:
            while a != ROOT and (b == ROOT or rank[a] > rank[b]):
                a = idom[a]
            while b != ROOT and (a == ROOT or rank[b] > rank[a]):
                b = idom[b]
        return a

    changed = True
    while changed:
        changed = False
        for node in order:
            if node in entries:
                continue
            done = [p for p in predecessors.get(node, []) if p in idom]
            if not done:
                continue
            new = done[0]
            for p in done[1:]:
                new = intersect(p, new)
            if idom.get(node) != new:
                idom[node] = new
                changed = True
    return [idom.get(at, -2) for at in range(len(text))]


def _dominates(idom: list[int], a: int, b: int) -> bool:
    while b >= 0:
        if a == b:
            return True
        b = idom[b]
    return False


def natural_loops(text: list, entries: list[int] | None = None) -> list[tuple[int, list[int]]]:
    """(header, member indices) per loop header, back edges to it merged.

    A back edge, taken or falling through, goes to an instruction that
    dominates it; its loop is the header and every instruction that reaches
    the edge's source without passing the header."""
    where = {one.ip: index for index, one in enumerate(text)}
    idom = dominators(text, entries if entries is not None else [0] if text else [])
    predecessors: dict[int, list[int]] = {}
    for index, one in enumerate(text):
        for successor in _successors(one, where):
            predecessors.setdefault(successor, []).append(index)
    bodies: dict[int, set[int]] = {}
    edges = [(index, header) for index, one in enumerate(text) for header in _successors(one, where)]
    for index, header in edges:
        if not _dominates(idom, header, index):
            continue
        body = bodies.setdefault(header, {header})
        pending = [index]
        while pending:
            at = pending.pop()
            if at in body:
                continue
            body.add(at)
            pending += predecessors.get(at, [])
    return sorted((header, sorted(body)) for header, body in bodies.items())


def _memory_operands(one) -> int:
    if isinstance(one, msp430.Insn):
        return one.memory
    return sum(
        1
        for operand in range(one.op_count)
        if one.op_kind(operand) == ix.OpKind.MEMORY and one.mnemonic != ix.Mnemonic.LEA
    )


def _named(code: Code, text: list, names: list[str] | None) -> dict[int, str]:
    """Procedure starts by offset: the given or public names, and every call
    target, unnamed ones as `sub_OFFSET`, so a static procedure's loops are
    not charged to the public one before it."""
    if names:
        formatter = ix.Formatter(ix.FormatterSyntax.MASM)
        starts = _entered(text, formatter)
        starts = ([0] if len(starts) < len(names) else []) + starts
        named = dict(zip(starts, names))
    else:
        named = dict(code.publics)
    for one in text:
        if one.flow_control == ix.FlowControl.CALL:
            target = _target(one)
            if target is not None and target not in named:
                named[target] = f"sub_{target:04x}"
    return named


def loops(data: bytes, names: list[str] | None = None, calls: bool = False,
          procedures: list[str] | None = None, bits: int | None = None) -> list[Loop]:
    out, seen = [], {}
    formatter = ix.Formatter(ix.FormatterSyntax.MASM)
    for code in images(data, procedures, bits):
        start = MODULE_CODE if code.data[:2] == b"bl" else 0
        text = _decoded(code.data, [start, *code.publics], code.arch, code.bits)
        named = _named(code, text, names)
        offsets = sorted(named)
        where = {one.ip: index for index, one in enumerate(text)}
        called = [_target(one) for one in text if one.flow_control == ix.FlowControl.CALL]
        entries = [where[at] for at in [start, *code.publics, *named, *called] if at in where]
        found = natural_loops(text, list(dict.fromkeys(entries)))
        headers = {header for header, _ in found}
        for header, members in found:
            # innermost: no other loop's header inside
            if any(other in headers and other != header for other in members):
                continue
            body = [text[at] for at in members]
            called = sum(1 for one in body if one.flow_control in CALLS)
            if called and not calls:
                continue
            owner = bisect.bisect_right(offsets, text[header].ip) - 1
            name = named[offsets[owner]] if owner >= 0 else "?"
            count = seen.get(name, 0)
            seen[name] = count + 1
            lines = [f"{one.ip:04x}  {one.text if code.arch == 'msp430' else formatter.format(one)}" for one in body]
            latch = max(members, key=lambda at: text[at].ip if header in _successors(text[at], where) else -1)
            out.append(Loop(f"{name}#{count}", text[header].ip, lines, sum(map(_memory_operands, body)), called,
                            code.arch, body, text[header].ip, text[latch].ip))
    return out


def procedures(data: bytes, procedures_: list[str] | None = None, bits: int | None = None) -> dict[str, list]:
    """Each procedure's reachable instructions, in address order."""
    out: dict[str, list] = {}
    for code in images(data, procedures_, bits):
        start = MODULE_CODE if code.data[:2] == b"bl" else 0
        text = _decoded(code.data, [start, *code.publics], code.arch, code.bits)
        named = _named(code, text, None)
        offsets = sorted(named)
        for one in text:
            owner = bisect.bisect_right(offsets, one.ip) - 1
            if owner >= 0:
                out.setdefault(named[offsets[owner]], []).append(one)
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("objects", nargs="+", type=Path)
    parser.add_argument("--names", type=Path, help="a --dump directory naming the procedures")
    parser.add_argument("--show", action="store_true", help="print each loop's instructions")
    parser.add_argument("--calls", action="store_true", help="keep loops that call")
    parser.add_argument("--bits", type=int, choices=(16, 32), help="the width of an ELF object's x86 code (32 unless said: gcc-ia16 objects are 16)")
    arguments = parser.parse_args()
    names = _dumped(arguments.names) if arguments.names else None
    runs = [{one.name: one for one in loops(path.read_bytes(), names, arguments.calls, bits=arguments.bits)} for path in arguments.objects]
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
