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
def test_a_rebuilt_object_keeps_every_code_fixup(obj: Path) -> None:
    """A fixup left behind is a field reading a bare zero at run time."""
    data = obj.read_bytes()
    out, why = wholeseg.rebuilt(data)
    if why != wholeseg.REBUILT:
        return
    before, after = module.of(omf.parse(data)), module.of(omf.parse(out))
    assert before is not None and after is not None
    was = [x for x in omf.fixups(omf.parse(data)) if x.seg == before.seg]
    now = [x for x in omf.fixups(omf.parse(out)) if x.seg == after.seg]
    assert len(now) == len(was), f"{obj.stem}: {len(was)} fixups became {len(now)}"


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
    """55 of the corpus's 125 objects. A canary on reach: the 70 refusals
    are all ops select.py cannot emit -- `add [bx+si],al`, which is VBDOS's
    own segment padding, and `mov ax,[si]`, whose address no fixup names."""
    done = sum(1 for obj in FIXTURES if wholeseg.rebuilt(obj.read_bytes())[1] == wholeseg.REBUILT)
    assert done == 55
