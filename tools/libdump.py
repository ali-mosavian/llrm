"""
One routine out of a DOS runtime library, disassembled.

qbopt/abi/runtime.py's contracts are meant to be established from evidence
rather than assumed, and its own standard is the QuickBASIC 4.5 source. For
the float routines there is no source to read: runtime/inc/rtmint.inc
declares B$FCMP, B$FILD and B$FIST and nothing in the 148-file runtime tree
defines them -- they live in the math library, which the source drop does
not carry. The shipped .LIB is the only evidence there is.

An OMF library is object modules laid end to end, each a THEADR followed by
its records and closed by a MODEND, with a dictionary after them. So
finding a routine is: walk the records, remember which module you are in,
and watch the PUBDEFs go by.

    uv run python tools/libdump.py B\\$FCMP
    uv run python tools/libdump.py B\\$FCMP --lib ~/path/to/BCOM45.LIB
"""

import sys
import argparse
from pathlib import Path
from dataclasses import dataclass

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from configs import QB45
from configs import PDS71
from configs import VBDOS
from iced_x86 import OpKind
from iced_x86 import Decoder
from iced_x86 import Mnemonic
from iced_x86 import Formatter
from iced_x86 import FlowControl
from iced_x86 import FormatterSyntax

from qbopt.objectfile import omf
from qbopt.frontend.declen import BITNESS

THEADR = 0x80
MODEND = (0x8A, 0x8B)
PUBDEF = (0x90, 0x91)

LIBS = {
    "qb45": QB45 / "LIB" / "BCOM45.LIB",
    "pds": PDS71 / "LIB" / "BCL71ENR.LIB",
    "vbdos": VBDOS / "LIB" / "VBDCL10E.LIB",
}


@dataclass(frozen=True, slots=True)
class Module:
    """One object module out of a library."""

    name: str
    records: list[omf.Record]

    def defines(self) -> dict[str, tuple[int, int]]:
        """Public symbol -> (segment index, offset)."""
        out: dict[str, tuple[int, int]] = {}
        for record in self.records:
            if record.type not in PUBDEF:
                continue
            body = record.body
            at = 0
            _group, at = omf._index(body, at)
            seg, at = omf._index(body, at)
            if seg == 0:
                at += 2  # a base frame follows only when the segment index is 0
            wide = record.type == 0x91
            while at < len(body) - 1:
                length = body[at]
                name = body[at + 1 : at + 1 + length].decode("latin-1")
                at += 1 + length
                offset = int.from_bytes(body[at : at + (4 if wide else 2)], "little")
                at += 4 if wide else 2
                _type, at = omf._index(body, at)
                out[name] = (seg, offset)
        return out


def modules(data: bytes) -> list[Module]:
    """Every object module in a library, or the one module in an OBJ."""
    archived = omf.library_modules(data)
    if archived:
        return [Module(name, records) for name, records in archived]
    records = omf.parse(data)
    headers = [record.body for record in records if record.type == THEADR]
    if len(headers) != 1 or not headers[0] or len(headers[0]) != headers[0][0] + 1:
        raise ValueError("standalone OMF object needs exactly one valid THEADR")
    return [Module(headers[0][1:].decode("latin-1"), records)]


def code_of(module: Module, seg: int) -> bytes:
    """The bytes of one segment, assembled from its LEDATA."""
    pieces: dict[int, bytes] = {}
    for _record, index, offset, payload in omf.ledata(module.records):
        if index == seg:
            pieces[offset] = payload
    if not pieces:
        return b""
    end = max(off + len(data) for off, data in pieces.items())
    out = bytearray(end)
    for off, data in sorted(pieces.items()):
        out[off : off + len(data)] = data
    return bytes(out)


def disassemble(code: bytes, start: int, limit: int = 400) -> list[str]:
    """A linear prefix, with explicit notice of branches omitted by its cutoff."""
    formatter = Formatter(FormatterSyntax.NASM)
    decoder = Decoder(BITNESS, code[start:], ip=start)
    out: list[str] = []
    visited, targets = set(), set()
    for insn in decoder:
        visited.add(insn.ip)
        if insn.flow_control in (
            FlowControl.CONDITIONAL_BRANCH,
            FlowControl.UNCONDITIONAL_BRANCH,
        ) and insn.op0_kind in (OpKind.NEAR_BRANCH16, OpKind.NEAR_BRANCH32, OpKind.NEAR_BRANCH64):
            targets.add(insn.near_branch_target)
        out.append(f"  {insn.ip:04x}  {code[insn.ip : insn.ip + insn.len].hex():<14} {formatter.format(insn)}")
        if insn.mnemonic in (Mnemonic.RET, Mnemonic.RETF) or len(out) >= limit:
            break
    if missing := targets - visited:
        out.append(
            "  ; linear listing has unvisited branch targets: " + ", ".join(f"{at:04x}" for at in sorted(missing))
        )
        out.append(
            "  ; use tools/contracts.py for reachable control flow and dependencies; this is not a complete contract"
        )
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="libdump")
    ap.add_argument("symbol")
    ap.add_argument("--lib", type=Path, action="append")
    ap.add_argument("--limit", type=int, default=60)
    args = ap.parse_args(argv)

    wanted = args.lib or [p for p in LIBS.values() if p.is_file()]
    for path in wanted:
        if not path.is_file():
            print(f"{path}: not there")
            continue
        found = [m for m in modules(path.read_bytes()) if args.symbol in m.defines()]
        if not found:
            print(f"{path.name}: {args.symbol} not defined here")
            continue
        for module in found:
            seg, offset = module.defines()[args.symbol]
            code = code_of(module, seg)
            print(f"\n=== {path.name}  module {module.name}  {args.symbol} at seg {seg}:{offset:#06x}")
            others = {n: o for n, (s, o) in module.defines().items() if s == seg}
            print(f"    also defines: {', '.join(sorted(others)) or '(nothing)'}")
            for line in disassemble(code, offset, args.limit):
                print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
