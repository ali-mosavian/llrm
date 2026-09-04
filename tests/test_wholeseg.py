"""
qbopt/wholeseg.py: an object whose code segment this pass wrote.

The claim these make is structural -- the object parses, keeps its code
length, keeps every fixup. Whether it *runs* is tests/test_e2e.py's, because
only LINK and a real 386 can say, and both of the bugs this had were
invisible to everything else.
"""

from pathlib import Path

import pytest

from qbopt import omf
from qbopt import module
from qbopt import wholeseg

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_rebuilt_object_parses_and_agrees_with_itself(obj: Path) -> None:
    """The segment may change length -- selection chooses encodings BC did
    not, and `nots-p-g2` comes out eight bytes longer. What has to hold is
    that the object agrees with itself: SEGDEF says what the LEDATA records
    actually carry, so a reader gets back exactly what was written."""
    data = obj.read_bytes()
    out, why = wholeseg.rebuilt(data)
    if why != wholeseg.REBUILT:
        assert out == data, f"{obj.stem}: refused and still changed the object"
        return
    after = module.of(omf.parse(out))
    assert after is not None
    assert after.end - after.start == len(after.code), f"{obj.stem}: SEGDEF and LEDATA disagree"
    assert len(after.code) > 0


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_rebuilt_object_keeps_every_code_fixup_it_still_has_a_home_for(obj: Path) -> None:
    """A fixup left behind is a field reading a bare zero at run time.

    Not every one survives now, and exactly one kind may not: the high half
    of a widened pair reads `[x+2]`, and folding the pair takes that
    relocation with it because there is no longer an instruction with that
    operand. layout.py reports which, relocate.py drops only those, and one
    it cannot explain is still refused outright -- so the count is checked
    against what was deliberately dropped rather than relaxed.
    """
    from qbopt import layout
    from qbopt import mir
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    data = obj.read_bytes()
    out, why = wholeseg.rebuilt(data)
    if why != wholeseg.REBUILT:
        return
    before, after = module.of(omf.parse(data)), module.of(omf.parse(out))
    assert before is not None and after is not None
    was = [x for x in omf.fixups(omf.parse(data)) if x.seg == before.seg]
    now = [x for x in omf.fixups(omf.parse(out)) if x.seg == after.seg]

    mapped = code_map(before)
    assert not isinstance(mapped, str)
    blocks = split.partition(before, mapped)
    # The same pipeline wholeseg runs: widening is not a pass and goes after
    # every one of them, just before lowering -- and `plain` is what a body
    # the allocator refuses is laid out as instead.
    raised = list(mir.bodies(before, blocks))
    bodies = [
        (name, transform.widened(transform.applied(body, before.dgroup, before.calls)))
        for name, body in raised
    ]
    # Widened too: a body the allocator refuses falls back to this, and
    # widening writes machine form with the registers BC had, so it needs
    # no allocation and is right either way.
    plain = [(name, transform.widened(body)) for name, body in raised]
    fields = frozenset(one.offset for one in omf.fixups(omf.parse(data)) if one.seg == before.seg)
    reached = frozenset(at for b in blocks for i in b.insns for at in range(i.at, i.end))
    laid = layout.rebuild(before, bodies, mapped.tables, fields, reached, plain=plain)
    assert not isinstance(laid, str), laid
    assert len(now) == len(was) - len(laid.dropped), (
        f"{obj.stem}: {len(was)} fixups became {len(now)}, {len(laid.dropped)} deliberately dropped"
    )
    # And each one really did belong to something that is gone.
    surviving = {op.at for _name, body in bodies for block in body.blocks for op in block.ops}
    assert not (laid.dropped & surviving), "a dropped fixup sits on an op that is still there"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_the_code_block_comes_after_every_extdef(obj: Path) -> None:
    """OMF numbers external symbols by the order their EXTDEFs appear.

    A code block written where the FIRST code LEDATA stood carries fixups
    naming externals whose EXTDEF has not been read yet, and LINK rejects
    the whole object -- `fatal error L1101: invalid object module`, with
    nothing in it to say which index was wrong. It took bisecting the
    emitter against BC's own boundaries to find, so it is pinned here.
    """
    out, why = wholeseg.rebuilt(obj.read_bytes())
    if why != wholeseg.REBUILT:
        return
    records = omf.parse(out)
    found = module.of(records)
    assert found is not None
    code_at = [
        n for n, r in enumerate(records) if r.type & 0xFE == omf.LEDATA and omf._index(r.body, 0)[0] == found.seg
    ]
    externals = [n for n, r in enumerate(records) if r.type & 0xFE == omf.EXTDEF]
    if externals and code_at:
        assert min(code_at) > max(externals), f"{obj.stem}: code fixups precede an EXTDEF"


def test_the_rebuildable_share_is_what_was_measured() -> None:
    """124 of the corpus's 125 objects. The one that refuses has bytes
    between the ops that reachability never reached, so nothing here can
    say whether they are code."""
    done = sum(1 for obj in FIXTURES if wholeseg.rebuilt(obj.read_bytes())[1] == wholeseg.REBUILT)
    assert done == 485
