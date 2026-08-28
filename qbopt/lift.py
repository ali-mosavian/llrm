#!/usr/bin/env python3
"""
The lift: BC's 16-bit instruction pairs read back as 32-bit values.

This is the step that turns instructions into meaning. BC keeps a long in
one of two register pairs, ax:dx or cx:bx, and every operation on it comes
out as two instructions -- the low half, then the high half with the carry
aware partner where there is one. Read as a pair, each is one 32-bit
operation on one value:

    mov cx,[X]   mov bx,[X+2]      v0 = load X
    and cx,[Y]   and bx,[Y+2]      v1 = v0 and Y
    mov dx,bx    mov ax,cx         a move -- ax:dx now holds v1
    xor ax,cx    xor dx,bx         v2 = v1 xor <whatever cx:bx holds>
    mov [W],ax   mov [W+2],dx      store v1 to W

Once the code is values rather than instructions the copies and the
reloads are visible as what they are, which is the whole point: matching
instruction patterns can never see that a pair-to-pair copy is dead.

Anything not recognised invalidates both pairs, because an instruction
this does not understand may write either of them.
"""

import struct
from pathlib import Path
from dataclasses import dataclass

# the five operations, low half -> (high half, widened)
PAIRS = {
    0x23: (0x23, 0x23, "and"),
    0x0B: (0x0B, 0x0B, "or"),
    0x33: (0x33, 0x33, "xor"),
    0x03: (0x13, 0x03, "add"),
    0x2B: (0x1B, 0x2B, "sub"),
}
# reg field -> (pair, half)   0 ax, 1 cx, 2 dx, 3 bx
REG = {0: (0, 0), 1: (1, 0), 2: (0, 1), 3: (1, 1)}

LOAD, ALUM, ALUV, STORE, MOVE = "load", "alu-m", "alu-v", "store", "move"
NEG = "neg"


@dataclass(slots=True)
class Value:
    op: str
    at: int
    end: int
    alu: str | None = None  # the mnemonic, which is what OPC is keyed on
    s1: int | None = None
    s2: int | None = None
    mem: int | None = None
    pair: int = 0
    src_pair: int = 0
    base: int = 0x06
    dlen: int = 2

    def __repr__(s) -> str:
        if s.op == LOAD:
            return f"load [{s.mem:#06x}]"
        if s.op == ALUM:
            return f"v{s.s1} {s.alu} [{s.mem:#06x}]"
        if s.op == ALUV:
            return f"v{s.s1} {s.alu} v{s.s2}"
        if s.op == MOVE:
            return f"move v{s.s1}"
        if s.op == NEG:
            return f"neg v{s.s1}"
        if s.op == STORE:
            return f"store v{s.s1} -> [{s.mem:#06x}]"
        return s.op


def decode1(b, i):
    """(kind, pair, srcpair, alu, mem) for the one instruction at i, or None.

    kind is 'ld' 'st' 'op' for the memory forms, 'mv' 'rr' for register to
    register, and half says which of the two it is."""
    # lift probes for the second half of a pair without knowing whether there
    # is one, so running off the end is ordinary and must answer "not a pair"
    # rather than raise.
    if i >= len(b):
        return None
    op = b[i]
    # A1 and A3 are ax with no ModRM at all, which is pair 0's low half --
    # saying "no half" here makes the pairing test compare against None and
    # the store silently fails to match its own high half.
    if op in (0xA1, 0xA3):
        if i + 3 > len(b):
            return None
        k = "ld" if op == 0xA1 else "st"
        return (k, 0, 0, 0, None, struct.unpack_from("<H", b, i + 1)[0], 3, 0x06, 2)
    if i + 1 >= len(b):
        return None
    m = b[i + 1]
    reg = (m >> 3) & 7
    if reg > 3:
        return None
    pair, half = REG[reg]
    # The memory forms BC uses for a long: a bare displacement for a static
    # variable, and bp relative for a local or a spilled temporary. Both are
    # carried through unchanged -- the widened instruction keeps the same
    # ModRM and displacement and only changes the register field, so nothing
    # here has to understand what the address means.
    base = m & 0xC7
    need = {0x06: 4, 0x46: 3, 0x86: 4}.get(base)
    if need is None or i + need > len(b):
        dlen = None
    elif base == 0x06:
        dlen, d = 2, struct.unpack_from("<H", b, i + 2)[0]
    elif base == 0x46:
        dlen, d = 1, struct.unpack_from("<b", b, i + 2)[0]
    else:
        dlen, d = 2, struct.unpack_from("<h", b, i + 2)[0]
    if dlen is not None:
        ln = 2 + dlen
        if i + ln > len(b):
            return None
        if op == 0x8B:
            return ("ld", pair, 0, half, None, d, ln, base, dlen)
        if op == 0x89:
            return ("st", pair, 0, half, None, d, ln, base, dlen)
        for lo, (hi, _w, _nm) in PAIRS.items():
            if op in (lo, hi):
                return ("op", pair, 0, half, op, d, ln, base, dlen)
        return None
    if (m & 0xC0) == 0xC0:  # register to register
        rm = m & 7
        if rm > 3:
            return None
        sp, sh = REG[rm]
        if sh != half:
            return None  # halves must match
        if op == 0x8B:
            return ("mv", pair, sp, half, None, None, 2, 0, 0)
        for lo, (hi, _w, _nm) in PAIRS.items():
            if op in (lo, hi):
                return ("rr", pair, sp, half, op, None, 2, 0, 0)
    return None


def lift(b, start, end):
    """Values, stores, and the instructions consumed, for one straight run."""
    vals, stores, cur = [], [], {0: None, 1: None}
    i = start

    def new(v):
        vals.append(v)
        return len(vals) - 1

    while i < end:
        # The long negate, which is three instructions and so does not fit the
        # pair shape at all:  neg lo / adc hi,0 / neg hi.  Not recognising it
        # is expensive out of all proportion to how often it appears -- it
        # invalidates both pairs, and every operation after it until the next
        # load then has no value to work from and falls out too.
        for pr, (lo, hi) in ((0, (0xD8, 0xDA)), (1, (0xD9, 0xDB))):
            if (
                i + 7 <= min(end, len(b))
                and b[i] == 0xF7
                and b[i + 1] == lo
                and b[i + 2] == 0x83
                and b[i + 3] == (0xD0 | (hi & 7))
                and b[i + 4] == 0
                and b[i + 5] == 0xF7
                and b[i + 6] == hi
                and cur[pr] is not None
            ):
                cur[pr] = new(Value(NEG, s1=cur[pr], at=i, end=i + 7, pair=pr))
                i += 7
                break
        else:
            pass
        if i >= end:
            break
        a = decode1(b, i)
        if a is None:
            cur[0] = cur[1] = None
            i += 1
            continue
        kind, pair, sp, half, alu, mem, ln, base, dlen = a
        nb = decode1(b, i + ln)
        ok = False
        if nb is not None:
            k2, p2, s2, h2, alu2, mem2, ln2, base2, dlen2 = nb
            same = k2 == kind and p2 == pair and s2 == sp and h2 is not None and h2 != half and base2 == base
            if same and kind in ("ld", "st", "op"):
                lo_d, hi_d = (mem, mem2) if half == 0 else (mem2, mem)
                lo_a, hi_a = (alu, alu2) if half == 0 else (alu2, alu)
                ok = hi_d == lo_d + 2
                if ok and kind == "op":
                    ok = lo_a in PAIRS and PAIRS[lo_a][0] == hi_a
                if ok:
                    if kind == "ld":
                        cur[pair] = new(Value(LOAD, mem=lo_d, at=i, end=i + ln + ln2, pair=pair, base=base, dlen=dlen))
                    elif kind == "op":
                        if cur[pair] is None:
                            ok = False
                        else:
                            cur[pair] = new(
                                Value(
                                    ALUM,
                                    alu=PAIRS[lo_a][2],
                                    s1=cur[pair],
                                    mem=lo_d,
                                    at=i,
                                    end=i + ln + ln2,
                                    pair=pair,
                                    base=base,
                                    dlen=dlen,
                                )
                            )
                    else:
                        if cur[pair] is None:
                            ok = False
                        else:
                            stores.append(
                                new(
                                    Value(
                                        STORE,
                                        s1=cur[pair],
                                        mem=lo_d,
                                        at=i,
                                        end=i + ln + ln2,
                                        pair=pair,
                                        base=base,
                                        dlen=dlen,
                                    )
                                )
                            )
            elif same and kind in ("mv", "rr"):
                if kind == "mv":
                    # A move is a value, not nothing. Dropping it looks
                    # tempting -- the halves are just being copied -- but the
                    # source pair is usually reused straight afterwards, so
                    # the copy is what keeps the value alive. And it cannot be
                    # left as BC wrote it either: mov ax,cx writes sixteen
                    # bits and leaves the top half of eax stale, which a
                    # widened store then writes out as garbage. So it becomes
                    # one 32-bit move, three bytes against four.
                    if cur[sp] is None:
                        ok = False
                    else:
                        cur[pair] = new(Value(MOVE, s1=cur[sp], at=i, end=i + ln + ln2, pair=pair, src_pair=sp))
                        ok = True
                else:
                    lo_a = alu if half == 0 else alu2
                    if cur[pair] is None or cur[sp] is None:
                        ok = False
                    else:
                        cur[pair] = new(
                            Value(
                                ALUV,
                                alu=PAIRS[lo_a][2],
                                s1=cur[pair],
                                s2=cur[sp],
                                at=i,
                                end=i + ln + ln2,
                                pair=pair,
                                src_pair=sp,
                            )
                        )
                        ok = True
        if ok:
            i += ln + ln2
        else:
            cur[0] = cur[1] = None
            i += 1
    return vals, stores


def report(vals, stores):
    """What the graph says is available, before anything is emitted."""
    mem = {}  # displacement -> value last stored there
    fwd = 0
    for v in vals:
        if v.op == STORE:
            mem[v.mem] = v.s1
        elif v.op == ALUM and v.mem in mem:
            fwd += 1
    return dict(values=len(vals), stores=len(stores), alu_v=sum(1 for v in vals if v.op == ALUV), forwardable=fwd)


if __name__ == "__main__":
    import sys

    exe, mp, lo, hi = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
    d = Path(exe).read_bytes()
    hdr = struct.unpack_from("<H", d, 8)[0] * 16
    b = d[hdr : hdr + 0x10000]
    vals, stores = lift(b, lo, hi)
    print(f"{len(vals)} values from {hi - lo} bytes\n")
    for n, v in enumerate(vals):
        print(f"   v{n:<3} @{v.at:<5} {v!r}")
    print()
    for k, x in report(vals, stores).items():
        print(f"   {k:14} {x}")


# ---------------------------------------------------------------------------
# From the graph to code.
#
# A value lives in the register pair BC computed it in, and stays there. That
# is the register allocation, and it is nearly free: BC already did it, and
# inheriting its choice means the operations need no moves between registers.
# What it buys is what BC could not express in 16 bits -- a pair-to-pair copy
# becomes a rebinding rather than two instructions, and a value fed to another
# pair is one instruction rather than two.

W32 = {0: "eax", 1: "ecx"}  # pair 0 low is ax, pair 1 low is cx
HI16 = {0: "dx", 1: "bx"}


def regions(vals):
    """Maximal runs of values whose instructions are contiguous in the code.
    Anything unlifted between two values ends a region -- it has to stay
    where it is, so the rewrite cannot span it."""
    out, cur = [], []
    for n, v in enumerate(vals):
        if cur and vals[cur[-1]].end != v.at:
            out.append(cur)
            cur = []
        cur.append(n)
    if cur:
        out.append(cur)
    return out


def needed(vals, regs=None):
    """Which values have to be computed.

    Stores write memory, so they always count. And whatever is left in a
    register when a *region* ends counts too: the code after the region was
    not lifted, so there is no telling what it reads, and BC routinely opens
    the next statement on the value the last one left in the pair. Asking
    this globally instead of per region drops exactly those values, and the
    statement that follows then reads one that was never computed."""
    need = [False] * len(vals)
    for n, v in enumerate(vals):
        if v.op == STORE:
            need[n] = True
    if regs is None:
        regs = regions(vals)
    for reg in regs:
        last = {}
        for n in reg:
            if vals[n].op != STORE:
                last[vals[n].pair] = n
        for n in last.values():
            need[n] = True
    for n in range(len(vals) - 1, -1, -1):
        if not need[n]:
            continue
        for s in (vals[n].s1, vals[n].s2):
            if s is not None:
                need[s] = True
    return need


def sizeof(v, need):
    """Bytes the widened form of this value costs."""
    if v.op in (LOAD, STORE) and v.pair == 0 and v.base == 0x06:
        return 4  # 66 A1 d / 66 A3 d
    if v.op == LOAD:
        return 3 + v.dlen  # 66 8B modrm d
    if v.op == ALUM:
        return 3 + v.dlen  # 66 op modrm d
    if v.op == ALUV:
        return 3  # 66 op C1 etc
    if v.op == MOVE:
        return 3  # 66 8B C1 etc
    if v.op == NEG:
        return 3  # 66 F7 D8 etc
    if v.op == STORE:
        return 3 + v.dlen
    return 0


def plan(vals, need):
    """What each region costs before and after, and whether it fits."""
    out = []
    for reg in regions(vals):
        before = vals[reg[-1]].end - vals[reg[0]].at
        after = sum(sizeof(vals[n], need) for n in reg if need[n])
        moves = sum(1 for n in reg if not need[n])
        out.append(
            dict(
                first=reg[0],
                last=reg[-1],
                at=vals[reg[0]].at,
                before=before,
                after=after,
                dead=moves,
                fits=(after <= before),
                n=len(reg),
            )
        )
    return out


OPC = {"and": 0x23, "or": 0x0B, "xor": 0x33, "add": 0x03, "sub": 0x2B}
LOREG = {0: 0, 1: 1}  # pair 0 low is eax (000), pair 1 is ecx (001)


def encode(v):
    """The widened form of one value. Register is the pair BC used."""
    o = OPC.get(v.alu, 0)
    disp = (
        (
            struct.pack("<b", v.mem)
            if v.dlen == 1
            else struct.pack("<h", v.mem)
            if v.mem < 0
            else struct.pack("<H", v.mem)
        )
        if v.mem is not None
        else b""
    )
    modrm = v.base | (LOREG[v.pair] << 3)
    if v.op == LOAD:
        if v.pair == 0 and v.base == 0x06:
            return bytes([0x66, 0xA1]) + disp
        return bytes([0x66, 0x8B, modrm]) + disp
    if v.op == STORE:
        if v.pair == 0 and v.base == 0x06:
            return bytes([0x66, 0xA3]) + disp
        return bytes([0x66, 0x89, modrm]) + disp
    if v.op == ALUM:
        return bytes([0x66, o, modrm]) + disp
    if v.op == ALUV:
        # mod 11, reg = destination pair's low register, rm = source's
        modrm = 0xC0 | (LOREG[v.pair] << 3) | LOREG[v.src_pair]
        return bytes([0x66, o, modrm])
    if v.op == MOVE:
        modrm = 0xC0 | (LOREG[v.pair] << 3) | LOREG[v.src_pair]
        return bytes([0x66, 0x8B, modrm])
    if v.op == NEG:
        return bytes([0x66, 0xF7, 0xD8 | LOREG[v.pair]])
    return b""


# Putting the high half back. The widened form writes only the 32-bit
# register, so dx and bx are left holding whatever they held before the
# region -- and BC reads them, because in its world that is where the top
# half of the long lives. Without liveness across blocks there is no telling
# whether anything after the region wants them, so they go back.
FIXUP = {
    0: bytes([0x66, 0x8B, 0xD0, 0x66, 0xC1, 0xEA, 0x10]),  # edx=eax, shr 16
    1: bytes([0x66, 0x8B, 0xD9, 0x66, 0xC1, 0xEB, 0x10]),
}  # ebx=ecx, shr 16


def emit_region(vals, need, reg):
    """Bytes for one region, plus the jump over whatever is left."""
    out = b""
    for n in reg:
        if need[n]:
            out += encode(vals[n])
    for p in sorted({vals[n].pair for n in reg if need[n]}):
        out += FIXUP[p]
    before = vals[reg[-1]].end - vals[reg[0]].at
    slack = before - len(out)
    if slack < 0:
        return None
    if slack == 1:
        out += b"\x90"
    elif slack >= 2:
        out += bytes([0xEB, slack - 2]) + b"\x90" * (slack - 2)
    return out
