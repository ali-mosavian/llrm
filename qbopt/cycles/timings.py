"""
Vendored from the runtime pass, which is where every cost figure in this project
was worked out.

    source  ~/work/badlogic/mgl, tools/cycles/timings.py
    taken   2026-08-29, unchanged but for this note and the import below

These are legacy approximate rankings, partly recalled from timing tables;
they are not uniformly verified latencies or reciprocal throughputs. Audited
form-specific bounds live in backend/timing.py; see docs/timing-audit.md.
DOSBox charges per instruction and does not validate these latency estimates.
"""

"""
Instruction costs for the machines this library actually runs on.

DOSBox is not one of them. It charges per instruction and models no latency
at all, which makes it blind to exactly the two things that decide whether
the qbext rewrites are worth doing: how expensive idiv really is, and what
BC's 16 bit register halves cost a P6 when 32 bit code reads them back.

THESE NUMBERS ARE APPROXIMATE and are the part of this tool most worth
arguing with. They are reciprocal throughput in cycles where the distinction
matters, taken from the vendors' optimisation guides and Agner Fog's tables
as far as memory of them goes; the ALU and mov entries are solid, the divide
entries are the roughest, and K5 is the least certain column here. Correct
them in place -- everything else reads from this table.

Ranges are given as a single representative figure. Where a cost depends on
the operand (idiv especially) the figure is for a 32 bit register operand.
"""

ARCHS = ["486", "P5", "P6", "K5", "K6", "K7", "Core"]

#                      486   P5    P6    K5    K6    K7   Core
COST = {
    "alu_rr": (1, 1, 1, 1, 1, 1, 1),  # add/and/or/xor/sub/cmp reg,reg
    "alu_rm": (2, 2, 1, 1, 1, 1, 1),  # ... reg,[mem]
    "alu_mr": (3, 3, 1, 1, 1, 1, 1),  # ... [mem],reg
    "mov_rr": (1, 1, 1, 1, 1, 1, 1),
    "mov_rm": (1, 1, 1, 1, 1, 1, 1),
    "mov_mr": (1, 1, 1, 1, 1, 1, 1),
    "mov_ri": (1, 1, 1, 1, 1, 1, 1),
    "shift_ri": (2, 1, 1, 1, 1, 1, 1),  # shl/shr/sar reg,imm
    "movzx": (3, 3, 1, 1, 1, 1, 1),
    "cdq": (3, 2, 1, 1, 1, 1, 1),
    "imul_r32": (26, 10, 4, 4, 3, 5, 3),  # 486 is 13-42, data dependent
    "imul_m32": (27, 11, 4, 4, 3, 5, 3),
    "idiv_r32": (43, 46, 39, 42, 41, 40, 26),  # the expensive one
    "idiv_m32": (44, 47, 39, 42, 41, 40, 26),
    "push_r": (1, 1, 1, 1, 1, 1, 1),
    "push_m": (4, 2, 2, 2, 2, 2, 2),
    "push_i": (1, 1, 1, 1, 1, 1, 1),
    "pop_r": (4, 1, 1, 1, 1, 1, 1),
    # POP memory is both the implicit stack load and an explicit store. 486
    # uses Intel's six-clock form; P5, P6, K6, K7 and Core follow GCC's
    # pentium, ppro, k6, athlon and core2 scheduling descriptions. GCC has
    # no distinct K5 machine description, so that column conservatively uses
    # the documented K6 three-cycle form rather than the register cost.
    "pop_m": (6, 1, 4, 3, 3, 4, 4),
    "pop_seg": (3, 3, 8, 3, 3, 5, 8),  # segment load, costly on P6+
    "mov_seg_r": (3, 3, 8, 3, 3, 5, 8),
    "les": (6, 4, 9, 4, 4, 6, 9),
    "nop": (1, 1, 1, 1, 1, 1, 1),
    "jmp_short": (3, 1, 1, 1, 1, 1, 1),  # predicted taken
    "jcc": (3, 1, 1, 1, 1, 1, 1),  # predicted
    "call_far": (18, 4, 21, 4, 4, 5, 22),  # real mode, no gate
    "ret_far": (13, 4, 17, 4, 4, 5, 18),
    "unknown": (2, 2, 2, 2, 2, 2, 2),
}

# A 16 bit write followed by a 32 bit read of the same register stalls the
# P6 core badly while it merges the halves; AMD and the 486/P5 do not care,
# and Core recovers most of it with a merging uop. This is the effect that
# makes BC's ax/dx halves expensive on exactly the machines qbext targets,
# and it is invisible to DOSBox.
#                       486  P5   P6   K5   K6   K7  Core
PARTIAL_STALL = (0, 0, 7, 0, 1, 1, 2)

# A 66h operand size prefix on an instruction that also carries an immediate
# changes its length and stalls the decoder on P6 and Core.
#                       486  P5   P6   K5   K6   K7  Core
LCP_STALL = (0, 0, 6, 0, 0, 0, 3)

# ---------------------------------------------------------------------------
# Pipelining
#
# The costs above are what an instruction occupies. What it *delays* is a
# different number, and on everything after the 486 the two come apart: an
# imul the next instruction does not need is nearly free, and one it does
# need costs its whole latency. So each kind also carries a latency, and a
# sequence is scored as the larger of what it occupies and what it depends
# on.
#
#                      486   P5    P6    K5    K6    K7   Core
LATENCY = {
    "alu_rr": (1, 1, 1, 1, 1, 1, 1),
    "alu_rm": (2, 2, 4, 3, 3, 3, 4),  # + load
    "alu_mr": (3, 3, 4, 3, 3, 3, 4),
    "mov_rr": (1, 1, 1, 1, 1, 1, 1),
    "mov_rm": (1, 1, 3, 2, 2, 3, 4),
    "mov_mr": (1, 1, 3, 2, 2, 3, 3),
    "mov_ri": (1, 1, 1, 1, 1, 1, 1),
    "shift_ri": (2, 1, 1, 1, 1, 1, 1),
    "movzx": (3, 3, 1, 1, 1, 1, 1),
    "cdq": (3, 2, 1, 1, 1, 1, 1),
    "imul_r32": (26, 10, 4, 4, 3, 5, 3),
    "imul_m32": (27, 11, 7, 6, 5, 7, 6),
    "mul_r16": (13, 11, 4, 4, 3, 5, 3),
    "idiv_r32": (43, 46, 39, 42, 41, 40, 26),
    "idiv_m32": (44, 47, 39, 42, 41, 40, 26),
    "div_r16": (24, 25, 23, 24, 24, 24, 22),
    "push_r": (1, 1, 1, 1, 1, 1, 1),
    "push_m": (4, 2, 3, 2, 2, 2, 3),
    "push_i": (1, 1, 1, 1, 1, 1, 1),
    "pop_r": (4, 1, 3, 2, 2, 2, 3),
    "pop_m": (6, 1, 4, 3, 3, 4, 4),
    "pop_seg": (3, 3, 8, 3, 3, 5, 8),
    "mov_seg_r": (3, 3, 8, 3, 3, 5, 8),
    "les": (6, 4, 9, 4, 4, 6, 9),
    "nop": (1, 1, 1, 1, 1, 1, 1),
    "jmp_short": (3, 1, 1, 1, 1, 1, 1),
    "jcc": (3, 1, 1, 1, 1, 1, 1),
    "call_far": (18, 4, 21, 4, 4, 5, 22),
    "ret_far": (13, 4, 17, 4, 4, 5, 18),
    "lahf": (3, 2, 3, 2, 2, 2, 3),
    "sahf": (2, 2, 1, 1, 1, 1, 1),
    "unknown": (2, 2, 2, 2, 2, 2, 2),
}
for k, v in COST.items():
    LATENCY.setdefault(k, v)
COST.setdefault("mul_r16", (13, 11, 4, 4, 3, 5, 3))
COST.setdefault("div_r16", (24, 25, 23, 24, 24, 24, 22))
COST.setdefault("lahf", (3, 2, 3, 2, 2, 2, 3))
COST.setdefault("sahf", (2, 2, 1, 1, 1, 1, 1))

# How many instructions can retire per cycle. The 486 is one at a time; the
# P5 pairs two when the rules allow; everything later is out of order and
# wide enough that the dependency chain, not the count, is usually the limit.
#              486  P5   P6   K5   K6   K7  Core
ISSUE = (1, 2, 3, 4, 3, 3, 4)

# Out of order machines can hide latency behind other work; in order ones
# cannot. Only the first two are in order.
INORDER = (1, 1, 0, 0, 0, 0, 0)

# lea, which is the reason strength reduction is worth considering at all:
# it does a shift and an add and does not touch the flags. In 16-bit code it
# needs a 67h address-size prefix to get at the scaled-index forms, and that
# prefix is not free on Intel -- see LCP below.
#                      486   P5    P6    K5    K6    K7   Core
COST["lea"] = (2, 1, 1, 1, 1, 1, 1)
LATENCY["lea"] = (2, 1, 1, 1, 1, 1, 1)

# A prefix is not free on the in-order parts: the 486 and the P5 spend about
# a clock decoding each one. That matters here more than anywhere, because
# every widened instruction carries a 66h -- the code segment is 16-bit and
# there is no way to make it otherwise in real mode, so a 32-bit operation
# always costs one prefix that the equivalent 16-bit operation does not.
#
# It puts a floor under "a long should cost what an integer costs": on a 486
# the answer is integer plus one clock per instruction, and no amount of
# analysis removes it.
#                      486   P5    P6    K5    K6    K7   Core
PREFIX = (1, 1, 0, 0, 0, 0, 0)

# REP STOS: a setup, then a clock count per cell. The 386 and 486 figures
# are Intel's (5+5n, 7+4n); the rest are Agner Fog's small-count rankings,
# where fast strings have not yet paid for their startup.
#                      486   P5    P6    K5    K6    K7   Core
COST["rep_stos"] = (7, 9, 30, 10, 10, 15, 30)
COST["rep_stos_cell"] = (4, 1, 1, 1, 1, 1, 1)
LATENCY["rep_stos"] = COST["rep_stos"]
LATENCY["rep_stos_cell"] = COST["rep_stos_cell"]

# 32-bit multiply, for telling it from the 16-bit one
#                      486   P5    P6    K5    K6    K7   Core
COST["mul_r32"] = (26, 10, 4, 4, 3, 5, 3)
LATENCY["mul_r32"] = (26, 10, 4, 4, 3, 5, 3)

# Complete the forms emitted by the C frontend's x87 lowering.  These remain
# target-ranking units, not a claim that all columns are measured cycles.  The
# Intel/AMD columns follow the corresponding GCC tuning tables; K5 uses the
# double-real forms in AMD publication 20007D where that guide lists them.
# K5's guide does not give FDIV a timing, so its division entry deliberately
# keeps the conservative K6 ranking rather than manufacturing a precise claim.
#
#                         486  P5  P6  K5  K6  K7 Core
_X87_LOAD = (8, 2, 2, 6, 6, 4, 6)
# 486 is the published four-clock form. Pentium occupies one issue slot but
# can pair FXCH with suitable floating work; P6/Core report zero added
# execution latency. GCC's Athlon description records two cycles, and K5/K6
# conservatively use the corresponding AMD rankings.
_X87_EXCHANGE = (4, 1, 0, 2, 2, 2, 0)
_X87_STORE = (8, 4, 4, 6, 4, 6, 6)
_X87_ADD = (8, 3, 3, 5, 2, 4, 3)
_X87_MUL = (16, 3, 5, 8, 2, 4, 5)
_X87_DIV = (73, 39, 56, 56, 56, 24, 24)

# Memory arithmetic is a distinct form.  K5 has direct published double-real
# figures (7-cycle add, 10-cycle multiply); the other ranking tables expose an
# arithmetic cost and a load cost, which are composed here and named as such.
_X87_ADD_M = (16, 5, 5, 7, 8, 8, 9)
_X87_MUL_M = (24, 5, 7, 10, 8, 8, 11)
_X87_DIV_M = (81, 41, 58, 62, 62, 28, 30)

COST.update(
    {
        # LEAVE is the target's ordinary frame-register move plus POP ranking.
        "leave": (5, 2, 2, 2, 2, 2, 2),
        "x87_load": _X87_LOAD,
        "x87_exchange": _X87_EXCHANGE,
        "x87_store": _X87_STORE,
        # Conversion plus store, except K5 whose FISTP int64 form is published
        # directly as seven cycles.
        "x87_convert_store": (35, 7, 7, 7, 6, 12, 12),
        "x87_add": _X87_ADD,
        "x87_add_m": _X87_ADD_M,
        "x87_mul": _X87_MUL,
        "x87_mul_m": _X87_MUL_M,
        "x87_div": _X87_DIV,
        "x87_div_m": _X87_DIV_M,
        # Control-word transfers are memory transfers in this ranking until a
        # primary form-specific table establishes a different value.
        "x87_control_load": _X87_LOAD,
        "x87_control_store": _X87_STORE,
    }
)

# Keep dependency cost separate in representation.  The available compiler
# tuning data does not establish a second number for these forms, so matching
# values are explicit instead of silently falling through to `unknown`.
for _form in (
    "leave",
    "x87_load",
    "x87_exchange",
    "x87_store",
    "x87_convert_store",
    "x87_add",
    "x87_add_m",
    "x87_mul",
    "x87_mul_m",
    "x87_div",
    "x87_div_m",
    "x87_control_load",
    "x87_control_store",
):
    LATENCY[_form] = COST[_form]
