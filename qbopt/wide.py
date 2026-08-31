"""
The 32-bit operation BC had to write as two, found as a graph and not as a
shape.

BC targets an 8086, so a long add is `add` on the low half and `adc` on the
high, and the carry between them is the only thing making the second
instruction meaningful. lift.py already recognises this, by looking for the
two instructions at adjacent addresses operating on adjacent operands. That
works and it is what the pass ships today, but it is a pattern match on
layout: it sees the pair because BC wrote them next to each other.

In SSA the carry is an edge. `add` defines a flags value and `adc` reads
that exact value, so the pair is found by asking a question about the graph
-- "which adc reads the flags this add produced" -- and the answer does not
depend on where either instruction sits, what lies between them, or whether
anything was reordered first. That is the whole reason to have raised at
all, and this is the first thing to use it.

It is also the argument that flags need not exist above this layer.
Measured across fixtures/omf and bench/nbody.bas, of 6,129 flag values only
192 are read by an adc or sbb and about 945 by a branch. The rest are
artefacts of modelling flags as a variable at all -- 3,133 "read" by a call,
because ir.Effects conservatively says a call may read any register, and
2,447 more carried through phis at joins for the same reason. Fold the 192,
turn the branches into a comparison that yields a value, and there is
nothing left for a flag to be.

What is refused, and why the check is not merely defensive: the two halves
have to be halves of the same long. Their memory operands must be two bytes
apart, their immediates must compose, and a register operand must pair with
the register the other half used. lift.py's own PAIRED table is the same
rule; getting it wrong does not fail loudly, it computes a different number.
"""

from dataclasses import dataclass

from qbopt import ir
from qbopt import mir
from qbopt.module import Addr

# The low half's operation, and what one 32-bit operation it becomes with
# its own high half. `neg` is here because BC's own 32-bit negate is
# `neg ax / adc dx,0 / neg dx` -- the adc carries the borrow, and the
# second neg is the high half, so the trio is one operation.
FOLDS = {
    ("add", "adc"): "add",
    ("sub", "sbb"): "sub",
    ("neg", "adc"): "neg",
}


@dataclass(frozen=True, slots=True)
class Pair:
    """Two instructions that are one 32-bit operation."""

    op: str  # what it computes, machine-independently
    low: mir.Op  # the half that produced the carry
    high: mir.Op  # the half that consumed it
    carry: mir.Value  # the flags value joining them

    @property
    def at(self) -> tuple[int, int]:
        return (self.low.at, self.high.at)


def _one_address(op: mir.Op) -> Addr | None:
    """The single named address this half reads, or None."""
    cells = [cell.addr for cell in op.loads if cell.addr is not None]
    return cells[0] if len(cells) == 1 else None


def _halves_agree(low: mir.Op, high: mir.Op) -> bool:
    """Whether the two really are the low and high halves of one long.

    Memory is the case that can be checked: the high half reads the two
    bytes above the low half's, which is lift.py's own `+2` and fails the
    same silent way if dropped. Where neither half names memory -- BC's
    negate, whose high half takes an immediate, or a register form -- there
    is no address to compare and the carry edge is the whole evidence.
    That is weaker, so it is stated rather than hidden: the pair is real
    because the flags value is, and nothing here claims more.
    """
    lo, hi = _one_address(low), _one_address(high)
    if lo is None and hi is None:
        return True
    if lo is None or hi is None:
        return False
    return hi == lo.plus(2)


def pairs(body: mir.MirBody) -> tuple[Pair, ...]:
    """Every 32-bit operation this body computes as two instructions."""
    produced: dict[mir.Value, mir.Op] = {}
    for block in body.blocks:
        for op in block.ops:
            for value in op.defines:
                if value.flags:
                    produced[value] = op

    found: list[Pair] = []
    for block in body.blocks:
        for op in block.ops:
            carried = [one for one in op.uses if one.flags]
            if len(carried) != 1:
                continue
            low = produced.get(carried[0])
            if low is None:
                continue
            whole = FOLDS.get((low.name, op.name))
            if whole is None or not _halves_agree(low, op):
                continue
            found.append(Pair(whole, low, op, carried[0]))
    return tuple(found)


def freed(body: mir.MirBody, folded: tuple[Pair, ...]) -> tuple[int, int]:
    """(flag values before, after folding these pairs).

    A folded pair's carry has one reader, so folding retires it. What the
    number is really measuring is whether flags are an abstraction this
    layer needs or one it inherited -- see the module docstring.
    """
    before = sum(1 for block in body.blocks for op in block.ops for v in op.defines if v.flags)
    return before, before - len(folded)


def refused(body: mir.MirBody) -> tuple[tuple[mir.Op, mir.Op, str], ...]:
    """Carry edges that look like a pair and are not one, with the reason.

    Reported rather than silently skipped: a pair this declines is either a
    shape FOLDS does not name yet or two halves whose operands disagree,
    and the two want different answers.
    """
    produced: dict[mir.Value, mir.Op] = {}
    for block in body.blocks:
        for op in block.ops:
            for value in op.defines:
                if value.flags:
                    produced[value] = op

    out: list[tuple[mir.Op, mir.Op, str]] = []
    for block in body.blocks:
        for op in block.ops:
            if op.op is ir.Operation.BARRIER:
                continue
            carried = [one for one in op.uses if one.flags]
            if len(carried) != 1 or carried[0] not in produced:
                continue
            low = produced[carried[0]]
            if (low.name, op.name) not in FOLDS:
                continue
            if not _halves_agree(low, op):
                out.append((low, op, "the two halves do not name adjacent bytes"))
    return tuple(out)


# What a conditional jump is really asking, and of what kind of comparison.
# The unsigned tests are here to be recognised, not to be folded off a
# runtime compare: B$CPI4 rebuilds its answer out of unsigned flags and
# calls.py's SYNTHESISED already refuses a site whose CF is read afterwards,
# so pairing one with `jb`/`ja` would be reading a flag the routine only
# synthesised on the way to something else.
TESTS = {
    "je": ("eq", None),
    "jne": ("ne", None),
    "jl": ("lt", True),
    "jle": ("le", True),
    "jg": ("gt", True),
    "jge": ("ge", True),
    "jb": ("lt", False),
    "jbe": ("le", False),
    "ja": ("gt", False),
    "jae": ("ge", False),
}

# The runtime routine whose whole purpose is to leave a comparison in the
# flags. Every branch in this corpus that reads a call's flags reads this
# one's -- measured, 803 of 803 -- so a call is not a barrier between a
# comparison and the jump that reads it; it IS the comparison.
COMPARE = "B$CPI4"


@dataclass(frozen=True, slots=True)
class Test:
    """A branch and the comparison it is really asking about.

    `signed` is None for equality, which does not care, and False for an
    unsigned ordering. `through` names the runtime routine where the
    comparison was a call rather than an instruction -- BC has no 32-bit
    compare on an 8086, so a long comparison is B$CPI4 and a jump, which is
    two halves of one operation exactly as add and adc are.
    """

    compare: mir.Op
    branch: mir.Op
    test: str
    signed: bool | None
    through: str | None
    condition: mir.Value


def tests(body: mir.MirBody, calls: dict[int, str]) -> tuple[Test, ...]:
    """Every branch, with the comparison that set what it reads.

    The flags stop being a variable here: a comparison yields a value and
    the branch asks a question of it, which is what a machine without
    condition codes would have written in the first place.
    """
    produced: dict[mir.Value, mir.Op] = {}
    for block in body.blocks:
        for op in block.ops:
            for value in op.defines:
                if value.flags:
                    produced[value] = op

    found: list[Test] = []
    for block in body.blocks:
        for op in block.ops:
            if op.op is not ir.Operation.BRANCH:
                continue
            asked = TESTS.get(op.name)
            carried = [one for one in op.uses if one.flags]
            if asked is None or len(carried) != 1:
                continue
            source = produced.get(carried[0])
            if source is None:
                continue
            test, signed = asked
            through = calls.get(source.at) if source.op is ir.Operation.CALL else None
            if through is not None and through != COMPARE:
                continue  # some other call's flags are not a comparison
            if through is not None and signed is False:
                continue  # see TESTS: B$CPI4 only synthesised the signed ones
            found.append(Test(source, op, test, signed, through, carried[0]))
    return tuple(found)
