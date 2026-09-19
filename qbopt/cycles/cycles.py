"""
Vendored from the runtime pass, which is where every cost figure in this project
was worked out.

    source  ~/work/badlogic/mgl, tools/cycles/cycles.py
    taken   2026-08-29, unchanged but for this note and the import below

These are published latencies rather than measurements, so treat what they give
as a ranking. DOSBox charges per instruction and models no latency at all, which
is why this exists alongside it.

`CASES` below is no longer only what was vendored. The `mgl: *` entries
(renamed 2026-08-30 from `qbext`) price mgl's own design -- a call into a
small helper routine (`do_mui4` etc.) mgl itself injected into the runtime
image. That is not what this project's `qbopt/legacy/calls.py` emits, and the old
`qbext` naming invited exactly that confusion. The `*: qbopt absorbed, *`
entries are this project's own current output, taken directly from
`absorb()`/`dividing()`/`consume()` against a constructed `CallSite`, the way
`tests/test_calls.py` builds one, rather than hand-written. The `stock`
entries (plus the new `rmi4: stock`) were confirmed 2026-08-30 against the
real `B$MUI4`/`B$DVI4`/`B$CPI4`/`B$RMI4` bytes in VBDOS's `VBDCL10E.LIB`,
module `..\\rt\\helpi4.asm` (offset 0x1d00, 262 bytes). One finding from that
pass: `B$MUI4`/`B$DVI4`/`B$RMI4` there are each a 5-byte far jmp thunk into
`__aFlmul`/`__aFldiv`/`__aFlrem`, defined in sibling library modules
`lmul.asm`/`ldiv.asm`/`lrem.asm` of the same `.LIB`; only `B$CPI4` has its own
inline body in `helpi4.asm`. `mul: stock fast/full`, `cmp: stock` and
`div256: stock` all matched byte-for-byte against those real routines, traced
along their actual short-path execution; `rmi4: stock` is new, traced the
same way through `lrem.asm`.
"""

#!/usr/bin/env python3
"""
Score an instruction sequence against the machines this library targets.

    tools/cycles/cycles.py                 all the built in cases
    tools/cycles/cycles.py 66A15A0066A35E00

DOSBox cannot answer the question these rewrites raise. It charges the same
for every instruction and models no latency, so it sees a widened sequence
and BC's original as equal work and reports no difference -- which is what
it did. This counts the sequence instead, on a 486, a P5, a P6, K5/K6/K7 and
Core, where an idiv is forty cycles and a 16 bit register write followed by
a 32 bit read is a stall.

Costs live in timings.py and are approximate; argue with them there.
"""
import os
import re
import sys
import tempfile
import subprocess

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from qbopt.cycles.timings import COST
from qbopt.cycles.timings import ARCHS
from qbopt.cycles.timings import ISSUE
from qbopt.cycles.timings import PREFIX
from qbopt.cycles.timings import INORDER
from qbopt.cycles.timings import LATENCY
from qbopt.cycles.timings import LCP_STALL
from qbopt.cycles.timings import PARTIAL_STALL

W16 = {"ax", "bx", "cx", "dx", "si", "di", "bp", "sp"}
W32 = {"eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp"}
SEG = {"es", "cs", "ss", "ds", "fs", "gs"}
ALU = {"add", "adc", "sub", "sbb", "and", "or", "xor", "cmp", "test"}
# opcodes carrying a full-width immediate, whose length 66h therefore changes
IMM_FULL = {
    "05",
    "0d",
    "15",
    "1d",
    "25",
    "2d",
    "35",
    "3d",  # alu eAX, imm32
    "69",
    "81",
    "a9",
    "c7",
    "f7",
    "68",
}  # imul/alu/test/mov/push


def disasm(hexs):
    b = bytes.fromhex(hexs)
    with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as f:
        f.write(b)
        t = f.name
    out = subprocess.run(["ndisasm", "-b", "16", t], capture_output=True, text=True).stdout
    os.unlink(t)
    rows = []
    for line in out.splitlines():
        m = re.match(r"^[0-9A-F]+\s+([0-9A-F]+)\s+(\S+)\s*(.*)$", line)
        if m:
            rows.append((m.group(1), m.group(2).lower(), m.group(3).strip().lower()))
    return rows


def classify(mnem, ops, raw=""):
    a = [o.strip() for o in ops.split(",")] if ops else []
    dst = a[0] if a else ""
    src = a[1] if len(a) > 1 else ""
    mem = lambda o: "[" in o
    if mnem == "nop":
        return "nop"
    if mnem in ("fld", "fild"):
        return "x87_load"
    if mnem == "fxch":
        return "x87_exchange"
    if mnem in ("fst", "fstp"):
        return "x87_store"
    if mnem in ("fist", "fistp"):
        return "x87_convert_store"
    if mnem in ("fadd", "faddp", "fsub", "fsubr", "fsubp", "fsubrp"):
        return "x87_add_m" if any(mem(one) for one in a) else "x87_add"
    if mnem in ("fmul", "fmulp"):
        return "x87_mul_m" if any(mem(one) for one in a) else "x87_mul"
    if mnem in ("fdiv", "fdivr", "fdivp", "fdivrp"):
        return "x87_div_m" if any(mem(one) for one in a) else "x87_div"
    if mnem == "fldcw":
        return "x87_control_load"
    if mnem in ("fstcw", "fnstcw"):
        return "x87_control_store"
    if mnem == "leave":
        return "leave"
    if mnem.startswith("rep"):
        return "rep_string"
    if mnem in ("jmp",):
        return "jmp_short"
    if mnem.startswith("j"):
        return "jcc"
    if mnem in ("cwd", "cdq", "cbw"):
        return "cdq"
    if mnem == "les":
        return "les"
    if mnem == "lea":
        return "lea"
    if mnem == "push":
        return "push_m" if mem(dst) else ("push_i" if dst.isdigit() or dst.startswith("0x") else "push_r")
    if mnem == "pop":
        return "pop_seg" if dst in SEG else ("pop_m" if mem(dst) else "pop_r")
    if mnem == "movzx" or mnem == "movsx":
        return "movzx"
    if mnem == "xchg":
        return "alu_rr"
    # 16-bit and 32-bit divide are not the same instruction and the second
    # is much the slower -- 27 clocks against 43 on a 486. Deciding by the
    # mnemonic alone prices a widened idiv as though it were free, which is
    # exactly the question "why is long division still slower" turns on.
    wide = raw.lower().startswith("66")
    if mnem == "mul":
        return "mul_r32" if wide else "mul_r16"
    if mnem == "lahf":
        return "lahf"
    if mnem == "sahf":
        return "sahf"
    if mnem == "neg":
        return "alu_rr"
    if mnem == "inc" or mnem == "dec":
        return "alu_rr"
    if mnem == "imul":
        if not wide:
            return "mul_r16"
        return "imul_m32" if mem(dst) or mem(src) else "imul_r32"
    if mnem in ("div", "idiv"):
        if not wide:
            return "div_r16"
        return "idiv_m32" if mem(dst) else "idiv_r32"
    if mnem == "call":
        return "call_far"
    if mnem in ("ret", "retf"):
        return "ret_far"
    if mnem == "mov":
        if dst in SEG or src in SEG:
            return "mov_seg_r"
        if mem(dst):
            return "mov_mr"
        if mem(src):
            return "mov_rm"
        if src and (src.startswith("0x") or src.isdigit()):
            return "mov_ri"
        return "mov_rr"
    if mnem in ("shl", "shr", "sar", "rol", "ror", "rcl", "rcr"):
        return "shift_ri"
    if mnem in ALU:
        if mem(dst):
            return "alu_mr"
        if mem(src):
            return "alu_rm"
        return "alu_rr"
    return "unknown"


# Not everything can be issued one per cycle and forgotten. A divider is not
# pipelined, a far call is microcode, and a memory PUSH/POP is multiple memory
# operations. For these the cost column is already an occupancy/throughput
# ranking, so it is used as one; for everything else the machine's width is
# the limit.
NOTPIPE = {
    "idiv_r32",
    "idiv_m32",
    "div_r16",
    "call_far",
    "ret_far",
    "push_m",
    "pop_m",
    "pop_seg",
    "mov_seg_r",
    "les",
    "lahf",
    "sahf",
}


def regs_of(ops):
    """the register names an operand list touches, 16 and 32 bit alike"""
    out = set()
    for tok in re.findall(r"[a-z]+", ops or ""):
        if tok in W16 or tok in W32 or tok in SEG:
            out.add(tok[1:] if tok in W32 else tok)
    return out


def score(rows):
    """
    Cycles per arch, as the larger of two things: how long the sequence
    occupies the machine, and how long its dependency chain takes. On an in
    order machine those are the same thing and it is simply the sum. On an
    out of order one an expensive instruction nobody waits for is nearly
    free, which is the whole reason imul and idiv have to be told apart by
    what depends on them rather than by what they cost.
    """
    n = len(ARCHS)
    occupancy = [0.0] * n  # issue slots consumed
    ready = [dict() for _ in range(n)]  # reg -> cycle its value lands
    chain = [0] * n  # end of the critical path so far
    written16 = set()
    detail = []

    for raw, mnem, ops in rows:
        kind = classify(mnem, ops, raw)
        c = list(COST.get(kind, COST["unknown"]))
        lat = list(LATENCY.get(kind, LATENCY["unknown"]))
        notes = []
        a = [o.strip() for o in ops.split(",")] if ops else []
        srcs = regs_of(ops)
        dsts = regs_of(a[0] if a else "")

        # The x87 stack is implicit in the printed operands.  Without a
        # synthetic dependency, a dependent expression such as
        # FLD/FADDP/FMUL/FDIVP/FISTP appears fully parallel and gets priced as
        # only its slowest operation on an out-of-order target.  One token is
        # intentionally conservative: it preserves the accumulator chain
        # without pretending that this small scorer allocates all eight stack
        # positions.
        if kind == "x87_load":
            dsts.add("x87-stack")
        elif kind in {
            "x87_store",
            "x87_convert_store",
            "x87_add",
            "x87_add_m",
            "x87_mul",
            "x87_mul_m",
            "x87_div",
            "x87_div_m",
        }:
            srcs.add("x87-stack")
            dsts.add("x87-stack")

        for o in a:
            r = o.strip("[]+ ")
            if r in W32 and r[1:] in written16:
                for k in range(n):
                    c[k] += PARTIAL_STALL[k]
                    lat[k] += PARTIAL_STALL[k]
                notes.append("partial-reg")
                written16.discard(r[1:])
        # each prefix costs a decode clock on the in-order parts
        npfx = 0
        h = raw.lower()
        while h[:2] in ("66", "67", "26", "2e", "36", "3e", "64", "65", "f0", "f2", "f3"):
            npfx += 1
            h = h[2:]
        stall = [0] * n
        for k in range(n):
            c[k] += PREFIX[k] * npfx
            lat[k] += PREFIX[k] * npfx
        if raw.lower().startswith("67") or raw.lower()[2:4] == "67":
            for k in range(n):
                stall[k] += LCP_STALL[k]
            notes.append("lcp-addr")
        # 66h only stalls when it actually changes the immediate's length.
        # The imm8 forms -- 6B imul, 83 alu, C1 shift -- sign extend and are
        # the same length either way, so they are free. Getting this wrong
        # taxes every imul-by-small-constant and quietly argues for shifts.
        op = raw.lower()[2:4] if raw.lower().startswith("66") else ""
        if op in IMM_FULL or (op and op[0] == "b" and op[1] in "89abcdef"):
            for k in range(n):
                stall[k] += LCP_STALL[k]
            notes.append("lcp")

        for k in range(n):
            c[k] += stall[k]
            lat[k] += stall[k]
            start = max([ready[k].get(r, 0) for r in srcs] + [0])
            done = start + lat[k]
            chain[k] = max(chain[k], done)
            for r in dsts:
                ready[k][r] = done
            if INORDER[k] or kind in NOTPIPE:
                occupancy[k] += c[k]
            else:
                occupancy[k] += 1.0 / ISSUE[k] + stall[k]

        if a and a[0] in W16 and kind not in ("alu_mr", "mov_mr"):
            written16.add(a[0])
        if a and a[0] in W32:
            written16.discard(a[0][1:])
        detail.append((mnem + " " + ops, kind, c, notes))

    # Two regimes, and a sequence can be in either. Standing alone it waits
    # for its own dependencies and costs the chain. Surrounded by other work
    # -- a loop body, a run of these back to back -- the machine overlaps the
    # waiting and what is left is the issue slots. Reporting only the larger
    # of the two would say widening buys nothing, which is true of one AND in
    # isolation and false of the four in a row that actually appear.
    lone = [int(round(max(occupancy[k], chain[k]))) for k in range(n)]
    bulk = [round(occupancy[k], 1) for k in range(n)]
    return (lone, bulk), detail


# The two long pushes and the far call, which every crackable operation pays
# before the callee is even entered. Common to the stock routine and to ours,
# so both carry it and the inline forms carry nothing.
CALL4 = "66FF365C0066FF365A009AAAAAAAAA"

CASES = {
    # ---- c = a AND b -------------------------------------------------------
    # BC's own: two halves all the way through
    "and: BC halves": "A15A008B165C0023065600231658 00A35E0089166000".replace(" ", ""),
    # widened, with dx put back
    "and: widened": "66A15A00662306560066A35E00668BD066C1EA10",
    # widened, dx dropped because the next thing reloads both
    "and: widened, no dx": "66A15A00662306560066A35E00",
    # ---- c = ((a AND b) + a) XOR b -----------------------------------------
    "chain: BC halves": "A15A008B165C00230656002316580003065600131658003306560033165800A35E0089166000",
    "chain: widened": "66A15A0066230656006603065600663306560066A35E00",
    # ---- a long multiply ---------------------------------------------------
    # B$MUI4 when both high words are zero, which is the case it is written for.
    # Confirmed 2026-08-30 byte-for-byte against VBDCL10E.LIB's real __aFlmul
    # (B$MUI4 itself is a 5-byte far jmp thunk into it -- see module docstring).
    "mul: stock fast": CALL4 + "558BEC8B46088B4E0C0BC88B4E0A75098B4606F7E15DCA0800",
    # and when they are not: three 16 bit muls and the partial products. Also
    # confirmed byte-for-byte, as the taken branch spliced straight in.
    "mul: stock full": CALL4 + "558BEC8B46088B4E0C0BC88B4E0A750953F7E18BD88B4606F7660C03D88B4606F7E103D35B5DCA0800",
    # mgl's do_mui4 -- a call into a helper mgl itself injected into the
    # runtime image, one 32 bit imul, no cases. Not what qbopt's calls.py
    # emits -- see "mul: qbopt absorbed, *" below for that.
    "mul: mgl call": CALL4 + "558BEC668B460666F76E0A668BD066C1EA105DCA0800",
    # times 256, recognised and inlined by mgl's own runtime pass
    "mul: mgl pow2": "66A15A0066C1E008EB03909090",
    # qbopt's own absorb(), against a memory-resident operand: mov eax,[a] /
    # imul eax,[b] / restore. Emitted.code from a real CallSite, not hand-written.
    "mul: qbopt absorbed, memory": "66A10000660FAF0600006650585A",
    # qbopt's own consume(), both operands popped off the stack: pop eax /
    # pop ecx / imul eax,ecx / restore.
    "mul: qbopt absorbed, register": "66586659660FAFC16650585A",
    # ---- a long compare ----------------------------------------------------
    # B$CPI4: compare the halves, then rebuild the flags by hand. Confirmed
    # 2026-08-30 byte-for-byte against VBDCL10E.LIB's real inline body.
    "cmp: stock": CALL4 + "558BEC508B460C3B460875118B460A3B46069F250041D1E8D0E40AE09E585DCA0800",
    # mgl's do_cpi4 -- a call into mgl's own injected helper, not qbopt's.
    "cmp: mgl call": CALL4 + "558BEC6650668B460A663B460666585DCA0800",
    # the inline form mgl's own runtime pass generated, jump and padding included
    "cmp: mgl inlined": "66A15A00663B065C00EB03909090",
    # qbopt's own absorb() against a memory-resident operand: push eax / mov
    # eax,[a] / cmp eax,[b] / pop eax -- no restore, because B$CPI4's answer
    # is flags, not a register value (absorb() special-cases COMPARE for
    # exactly this; see test_an_absorbed_comparison_leaves_no_value_to_restore).
    "cmp: qbopt absorbed, memory": "665066A10000663B0600006658",
    # qbopt's own consume() -- compare_consume()'s heavier bp-relative form,
    # needed because a real call to B$CPI4 changes nothing but the flags and
    # both operands only exist on the stack here, never reloaded from memory.
    "cmp: qbopt absorbed, stack": "5566528BEC668B560A663B56068B560489560C668B56008D660C5D",
    # ---- dividing by 256 ---------------------------------------------------
    # B$DVI4. 256 has a zero high word, so this takes its short path: both
    # signs checked, then two 16 bit divs. The long path below it loops.
    # Confirmed 2026-08-30 byte-for-byte against VBDCL10E.LIB's real __aFldiv
    # (B$DVI4 itself is a 5-byte far jmp thunk into it), traced along this
    # exact execution path.
    "div256: stock": CALL4 + "558BEC57565333FF8B46080BC07D11"
    "8B460C0BC07D11"
    "0BC075158B4E0A8B460833D2F7F18BD8"
    "8B4606F7F18BD3EB38"
    "4F7507"
    "5B5E5F5DCA0800",
    # mgl's do_dvi4, which reaches a single idiv -- mgl's own injected helper.
    "div256: mgl call": CALL4 + "558BEC6651668B4606668B4E0A669966F7F9668BD066C1EA1066595DCA0800",
    # the generated shift stub mgl's own runtime pass produced
    "div256: mgl shift": CALL4 + "558BEC668B4606669966C1EA186603C266C1F808668BD066C1EA105DCA0800",
    # qbopt's own dividing(), against a memory-resident operand -- C's own
    # divide, no divisor test: mov eax,[a] / mov ecx,[b] / cdq / idiv ecx / restore.
    "div: qbopt absorbed, memory": "66A10000668B0E0000669966F7F96650585A",
    # qbopt's own consume(): pop eax / pop ecx / cdq / idiv ecx / restore.
    "div: qbopt absorbed, register": "66586659669966F7F96650585A",
    # ---- B$RMI4, MOD by 256 -------------------------------------------------
    # B$RMI4 shares B$DVI4's entry logic -- both are far jmp thunks into the
    # same library, B$RMI4 into __aFlrem (lrem.asm, sibling of ldiv.asm) --
    # but the two differ from just past the divide: B$RMI4 discards the
    # quotient and answers from the remainder in dx, and its own sign fixup
    # is a subtract-back correction rather than a plain negate. No case
    # existed here before 2026-08-30; this one is traced the same way
    # div256's was, straight through the real routine's short path.
    "rmi4: stock": CALL4 + "558BEC535733FF8B46080BC07D11"
    "8B460C0BC07D10"
    "0BC075188B4E0A8B460833D2F7F1"
    "8B4606F7F18BC233D2"
    "4F7943EB48"
    "5F5B5DCA0800",
    # qbopt's own dividing() with REMAINDER: the same load/idiv as divide,
    # plus mov eax,edx to answer from the remainder, then restore.
    "rmi4: qbopt absorbed, memory": "66A10000668B0E0000669966F7F9668BC26650585A",
    # qbopt's own consume(): pop eax / pop ecx / cdq / idiv ecx / mov eax,edx / restore.
    "rmi4: qbopt absorbed, register": "66586659669966F7F9668BC26650585A",
}

# B$DVI4's other half. When the divisor does not fit in 16 bits it shifts
# both operands right until it does, one bit per pass, and that loop is not
# something a straight line walk can price. Scored separately, per pass.
DVI4_LOOP = "D1EBD1D9D1EAD1D80BDB75F4"


def report(name, hexs):
    rows = disasm(hexs)
    total, detail = score(rows)
    return name, len(rows), total, detail


def table(title, which, cases):
    print()
    print(title)
    print(f"{'case':24}{'ins':>4}  " + "".join(f"{a:>7}" for a in ARCHS))
    print("-" * (28 + 7 * len(ARCHS)))
    for name, hexs in cases.items():
        _, cnt, tot, _ = report(name, hexs)
        print(f"{name:24}{cnt:>4}  " + "".join(f"{t:>7g}" for t in tot[which]))


if __name__ == "__main__":
    if len(sys.argv) > 1:
        n, cnt, tot, det = report("argv", sys.argv[1])
        print(f"{cnt} instructions")
        for txt, kind, c, notes in det:
            print(f"   {txt:32} {kind:12} {c} {' '.join(notes)}")
        print("   alone " + " ".join(f"{a}={t}" for a, t in zip(ARCHS, tot[0])))
        print("   bulk  " + " ".join(f"{a}={t:g}" for a, t in zip(ARCHS, tot[1])))
        sys.exit()

    table("standing alone -- the sequence waits for its own dependencies", 0, CASES)
    table("back to back -- the machine has other work to overlap the waiting", 1, CASES)
    _, n, t, _ = report("loop", DVI4_LOOP)
    print()
    print("B$DVI4's normalising loop, one pass of %d instructions:" % n)
    print("   " + "  ".join(f"{a}={c}" for a, c in zip(ARCHS, t[0])))
    print("   it runs once per significant bit of the divisor above 16, so a")
    print("   divisor near 2^31 costs fifteen of these on top of the figures")
    print("   above -- which are its short path, not its worst one.")
