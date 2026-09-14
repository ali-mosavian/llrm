"""
qbopt/frontend/wide.py's own gate: the pair is a graph property, and the check that
two halves belong together is the part that fails silently if it is wrong.
"""

from pathlib import Path

import pytest

import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.frontend import wide
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))
SOMEWHERE = Addr(Space.SEGMENT, 0x10, 5)


def half(name: str, addr: Addr | None) -> mir.Op:
    loads = (mir.MemRef(addr, 2),) if addr is not None else ()
    return mir.Op(0, ir.Operation.BINARY, name, (), (), loads, ())


def bodies(obj: Path) -> list[mir.MirBody]:
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    result = corpus.bodies(obj)
    if isinstance(result, str) or not partitioned:
        return []
    nodes = {ir.span(n)[0]: n for body in result for n in body.nodes}
    out = []
    for body in result:
        mine = [b for b in partitioned if any(lo <= b.at < hi for lo, hi in body.body.ranges)]
        if not mine:
            continue
        built = mir.raise_body(mine, nodes, body.body.seed)
        assert not isinstance(built, str), built
        out.append(built)
    return out


@pytest.mark.parametrize(
    ("low", "high", "agree", "why"),
    [
        (SOMEWHERE, SOMEWHERE.plus(2), True, "the high half is the two bytes above the low"),
        (SOMEWHERE, SOMEWHERE.plus(4), False, "four apart is not one long"),
        (SOMEWHERE, SOMEWHERE, False, "the same address twice is not two halves"),
        (SOMEWHERE, None, False, "one half in memory and one not is not a pair"),
        (None, None, True, "BC's own negate: the high half takes an immediate"),
    ],
)
def test_two_halves_belong_together_or_they_do_not(low: Addr | None, high: Addr | None, agree: bool, why: str) -> None:
    """Getting this wrong computes a different number rather than failing.

    lift.py's `+2` is the same rule, and this exists because a check that
    never refuses is not a check -- the corpus happens to refuse none, so
    the refusals have to be shown to be reachable here.
    """
    assert wide._halves_agree(half("add", low), half("adc", high)) is agree, why


def test_the_pair_is_found_through_the_carry_not_the_layout() -> None:
    """The point of raising: `add` defines a flags value and `adc` reads
    that exact value, so the pair is a question about the graph rather than
    about where BC put the two instructions."""
    seen = 0
    for body in bodies(Path("fixtures/omf/arith-v-g3.obj")):
        for pair in wide.pairs(body):
            assert pair.carry in pair.low.defines
            assert pair.carry in pair.high.uses
            seen += 1
    assert seen, "arith is a program of 32-bit arithmetic"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_folded_pair_is_a_shape_folds_names(obj: Path) -> None:
    for body in bodies(obj):
        for pair in wide.pairs(body):
            # By what the two halves compute, not by what x86 spells them.
            # The carry half of an add is an add; that it is written "adc"
            # is lowering's business and was this pass's key.
            assert (pair.low.kind, pair.high.kind) in wide.FOLDS
            assert pair.op == wide.FOLDS[(pair.low.kind, pair.high.kind)]


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_carry_has_exactly_one_producer(obj: Path) -> None:
    """SSA's own promise, relied on here: the flags value an adc reads was
    defined once, so "which add produced it" has one answer."""
    for body in bodies(obj):
        for pair in wide.pairs(body):
            makers = [op for block in body.blocks for op in block.ops if pair.carry in op.defines]
            assert makers == [pair.low]


def test_folding_retires_the_carry_it_consumed() -> None:
    for body in bodies(Path("fixtures/omf/arith-v-g3.obj")):
        found = wide.pairs(body)
        before, after = wide.freed(body, found)
        assert after == before - len(found)


def calls_of(obj: Path) -> dict[int, str]:
    found = corpus.loaded(obj)
    assert found is not None
    return found.calls


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_branch_finds_the_comparison_it_reads(obj: Path) -> None:
    """A branch with no comparison behind it would be reading a flag from
    nowhere, which would mean the SSA edges are wrong rather than that the
    program is odd. Measured: 871 of 871 across the corpus."""
    for body in bodies(obj):
        branches = [op for block in body.blocks for op in block.ops if op.op is ir.Operation.BRANCH]
        paired = {one.branch.at for one in wide.tests(body, calls_of(obj))}
        assert {op.at for op in branches} == paired


def test_a_long_comparison_is_a_call_and_a_jump() -> None:
    """BC has no 32-bit compare on an 8086, so a long comparison is
    B$CPI4 followed by a jcc -- two halves of one operation exactly as add
    and adc are. Every branch in this corpus that reads a call's flags
    reads that routine's."""
    seen = 0
    for body in bodies(Path("fixtures/omf/cmpord-v-g3.obj")):
        for one in wide.tests(body, calls_of(Path("fixtures/omf/cmpord-v-g3.obj"))):
            if one.through is None:
                continue
            assert one.through == wide.COMPARE
            assert one.signed is not False, "B$CPI4 only synthesised the signed answers"
            seen += 1
    assert seen, "cmpord is a program of long comparisons"


def test_an_unsigned_test_is_never_folded_off_the_long_compare() -> None:
    """calls.py refuses a B$CPI4 site whose CF is read afterwards, because
    the routine synthesised CF on its way to SF rather than meaning it.
    Pairing one with jb/ja would read exactly that flag.

    About B$CPI4 and not about runtime compares in general: B$FCMP is the
    other one and its flags are the unsigned half, which the next test says.
    """
    for obj in FIXTURES:
        for body in bodies(obj):
            for one in wide.tests(body, calls_of(obj)):
                if one.through == wide.COMPARE:
                    assert one.signed is not False


def test_a_signed_test_is_never_folded_off_the_float_compare() -> None:
    """The mirror of the rule above, and for the opposite reason.

    B$FCMP leaves the x87 status word in the flags through sahf, so the
    comparison arrives as CF and ZF -- the unsigned half. Microsoft's own
    runtime branches on it that way: runtime/rt/grwindow.asm uses JZ for
    equal and JC for less-than, twice, and BC emits `jbe` at every site in
    suite/fpemu.bas.
    """
    seen = 0
    for obj in FIXTURES:
        for body in bodies(obj):
            for one in wide.tests(body, calls_of(obj)):
                if one.through == "B$FCMP":
                    assert one.signed is not True
                    seen += 1
    assert seen, "fixtures/omf holds fpemu objects; their float branches should pair"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_nothing_reads_a_flag_once_both_folds_are_applied(obj: Path) -> None:
    """The whole argument for flags not existing above this layer.

    What is left over is not a flag anything asked for: a value nothing
    reads, or one "read" only by a call or carried through a phi, both of
    which exist because ir.Effects says a call may touch any register.
    """
    for body in bodies(obj):
        accounted = {one.carry for one in wide.pairs(body)}
        accounted |= {one.condition for one in wide.tests(body, calls_of(obj))}
        # a phi is recorded as None: it is a join, not an op, and both are
        # equally "not something that asked for a flag"
        consumers: dict[mir.Value, list[mir.Op | None]] = {}
        for block in body.blocks:
            for phi in block.phis:
                for value in phi.incoming.values():
                    consumers.setdefault(value, []).append(None)
            for op in block.ops:
                for value in op.uses:
                    consumers.setdefault(value, []).append(op)
        for block in body.blocks:
            for op in block.ops:
                for value in op.defines:
                    if not value.flags or value in accounted:
                        continue
                    who = consumers.get(value, [])
                    assert all(one is None or one.op is ir.Operation.CALL for one in who), (
                        f"{value} at {op.at:#06x} is read by something that is not a call or a phi"
                    )
