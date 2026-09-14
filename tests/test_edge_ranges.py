"""An array subscript guarded inside a loop has a tighter range only on that arm."""

from dataclasses import replace

import pytest

from qbopt.analysis import ranges
from qbopt.model import ir, mir


def guarded_loop():
    start, counter, advanced, offset = (mir.Value(n, at) for n, at in [(1, 0), (2, 10), (3, 40), (6, 30)])
    def compare(at, bound, yes, no):
        flags = mir.Value(at + 10, at, flags=True)
        test = mir.Op(at, ir.Operation.COMPARE, "", (flags,), (counter,), kind=mir.Kind.SUB,
                      args=(mir.Held(counter, 2), mir.Const(bound, 2)))
        branch = mir.Op(at+1, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH,
                        test=mir.Kind.LT, target=yes)
        return mir.MirBlock(at, (), (test, branch), (yes, no))
    initial = mir.Op(0, ir.Operation.MOVE, "", (start,), (), kind=mir.Kind.COPY,
                     args=(mir.Const(0, 2),), results=(mir.Held(start, 2),))
    header = replace(compare(10, 10, 20, 50), phis=(mir.Phi(counter, {0: start, 40: advanced}),))
    scaled = mir.Op(30, ir.Operation.BINARY, "", (offset,), (counter,), kind=mir.Kind.MUL,
                    args=(mir.Held(counter, 2), mir.Const(2, 2)), results=(mir.Held(offset, 2),))
    step = mir.Op(40, ir.Operation.UNARY, "", (advanced,), (counter,), kind=mir.Kind.INCREMENT,
                  args=(mir.Held(counter, 2),), results=(mir.Held(advanced, 2),))
    return mir.MirBody(0, (mir.MirBlock(0, (), (initial,), (10,)), header,
                           compare(20, 4, 30, 40), mir.MirBlock(30, (), (scaled,), (40,)),
                           mir.MirBlock(40, (), (step,), (10,)), mir.MirBlock(50, (), (), ())))


def test_guard_refines_subscript_without_leaking_to_the_join():
    """i<4 bounds a word-array offset to 0..6, not the whole loop's 0..18."""
    body = guarded_loop()
    counter = body.blocks[1].phis[0].result
    offset = body.blocks[3].ops[0].results[0].value
    known = ranges.bounded(body)
    assert known[30][counter] == ranges.Interval(0, 3, 2)
    assert known[30][offset] == ranges.Interval(0, 6, 2)
    assert known[40][counter] == ranges.Interval(0, 9, 2)
    assert counter not in known.get(50, {})


@pytest.mark.parametrize("test,successor,want", [
    (mir.Kind.LT, 30, (0, 3)), (mir.Kind.LT, 40, (4, 9)),
    (mir.Kind.LE, 30, (0, 4)), (mir.Kind.GT, 30, (5, 9)),
    (mir.Kind.GE, 30, (4, 9)), (mir.Kind.EQ, 30, (4, 4)),
    (mir.Kind.NE, 40, (4, 4)), (mir.Kind.NE, 30, (0, 9)),
])
def test_signed_comparison_edges(test, successor, want):
    block = guarded_loop().blocks[2]
    counter = block.ops[0].args[0].value
    block = replace(block, ops=(*block.ops[:-1], replace(block.ops[-1], test=test)))
    result = ranges.on_edge(block, successor, {counter: ranges.Interval(0, 9, 2)})
    assert result[counter] == ranges.Interval(*want, 2)


@pytest.mark.parametrize("operation,kind", [(ir.Operation.BINARY, mir.Kind.SUB), (ir.Operation.COMPARE, mir.Kind.AND)])
def test_non_comparison_flags_do_not_establish_a_bound(operation, kind):
    block = guarded_loop().blocks[2]
    counter = block.ops[0].args[0].value
    known = {counter: ranges.Interval(0, 9, 2)}
    block = replace(block, ops=(replace(block.ops[0], op=operation, kind=kind), block.ops[1]))
    assert ranges.on_edge(block, 30, known) == known


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_rngarm_writes_its_counter_only_after_the_loop(tag):
    """RNGARM printed 28,7,10 but stored INDEX eleven times; its guarded array cannot alias INDEX."""
    from pathlib import Path
    from qbopt import wholeseg
    from qbopt.analysis import loops
    from qbopt.objectfile import module, omf
    from qbopt.objectfile.module import Space
    data = Path(f"fixtures/regressions/rngarm-{tag}.obj").read_bytes()
    found = module.of(omf.parse(data))
    states = []
    def watch(stage, name, state):
        if stage == "mir-widen":
            states.append(state)
    result = wholeseg.emitted(data, watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body, = states
    inside = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
    writes = [(block.at, ref) for block in body.blocks for op in block.ops for ref in op.stores
              if ref.addr is not None and ref.addr.space is Space.SEGMENT
              and ref.addr.index == found.program_data and ref.addr.disp == 14]
    assert inside and len(writes) <= 1
    assert not {at for at, _ in writes} & inside
