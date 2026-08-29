"""
Vendored from the runtime pass, which is where every cost figure in this project
was worked out.

    source  ~/work/badlogic/mgl, tools/cycles/timings.py
    taken   2026-08-29, unchanged but for this note and the import below

These are published latencies rather than measurements, so treat what they give
as a ranking. DOSBox charges per instruction and models no latency at all, which
is why this exists alongside it.
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

# 32-bit multiply, for telling it from the 16-bit one
#                      486   P5    P6    K5    K6    K7   Core
COST["mul_r32"] = (26, 10, 4, 4, 3, 5, 3)
LATENCY["mul_r32"] = (26, 10, 4, 4, 3, 5, 3)
