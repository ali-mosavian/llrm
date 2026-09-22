"""Pre-allocation selection of complete far-pointer loads."""

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import farload
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def test_adjacent_words_are_one_far_load_only_when_the_high_word_is_a_selector() -> None:
    """qcport's dynamic far-struct fields emitted two loads per far pointer.

    Their dynamic owner has no allocation provenance, but the C type and exact
    adjacent effective addresses still prove that they are the two words of
    one far pointer.  Keeping the pair separate made allocation rematerialize
    the struct base for both halves; indexed.lru_use grew from 66 to 81
    instructions and acquired a spill slot.
    """
    first_value, second_value = mir.Value(1, 1), mir.Value(2, 1)
    first_ref = mir.MemRef(Addr(Space.FAR, 4), 2, typed=("pointer4", False))
    second_ref = mir.MemRef(Addr(Space.FAR, 6), 2, typed=("pointer4", False))

    def load(at: int, value: mir.Value, ref: mir.MemRef, cell: ir.Mem) -> lir.Insn:
        destination = ir.Held(value.id, 2)
        op = mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            (value,),
            (),
            loads=(ref,),
            kind=mir.Kind.LOAD,
            args=(mir.Cell(ref),),
            results=(mir.Held(value, 2),),
        )
        return lir.Insn(
            at,
            (at, at),
            ir.Semantics(ir.Operation.MOVE, "mov", (destination,), (cell,)),
            (value.id,),
            tuple(one.value for one in ir.values(cell)),
            op=op,
        )

    original = (
        load(1, first_value, first_ref, ir.Mem(first_ref.addr, 2, base=ir.Held(10, 2), selector=ir.Held(11, 2))),
        load(2, second_value, second_ref, ir.Mem(second_ref.addr, 2, base=ir.Held(10, 2), selector=ir.Held(11, 2))),
    )

    selected = farload.selected(original, frozenset({2}))

    assert selected[0].what is not None and selected[0].what.name == "les"
    assert selected[0].what.sources[0].width == 4
    assert selected[0].defines == (1, 2)
    assert selected[1].what is not None and selected[1].what.op is ir.Operation.NOTHING
    # Adjacent words whose high one is a number, not a selector, stay two loads.
    assert farload.selected(original, frozenset()) == original


def test_fixed_far_pointer_load_defers_fusion_until_after_allocation() -> None:
    """indexed.lru_use's fixed parameter pair became an eager LES.

    That fixed ES at the definition, so allocation copied ES to the long-lived
    FS value and stored the offset in a new frame slot.  A fixed BP-relative
    address cannot be split by allocation; retaining its independent words
    lets ordinary rematerialization act on either half, after which the final
    physical peephole can still select LES/LFS/LGS when the pair stays adjacent.
    """
    values = (mir.Value(1, 1), mir.Value(2, 1))

    def load(at: int, value: mir.Value, displacement: int) -> lir.Insn:
        ref = mir.MemRef(Addr(Space.FRAME, displacement), 2, typed=("pointer4", False))
        destination = ir.Held(value.id, 2)
        cell = ir.Mem(ref.addr, 2)
        op = mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            (value,),
            (),
            loads=(ref,),
            kind=mir.Kind.LOAD,
            args=(mir.Cell(ref),),
            results=(mir.Held(value, 2),),
        )
        return lir.Insn(
            at,
            (at, at),
            ir.Semantics(ir.Operation.MOVE, "mov", (destination,), (cell,)),
            (value.id,),
            (),
            op=op,
        )

    original = (load(1, values[0], 4), load(2, values[1], 6))

    assert farload.selected(original, frozenset({2})) == original


def _procedure(listing: str, name: str) -> str:
    return listing[listing.index(f"{name} proc") : listing.index(f"{name} endp")]


def _modern(fixture: str, name: str) -> str:
    from pathlib import Path

    from qbopt.backend import masm
    from qbopt.frontend.modern import driver
    from qbopt.frontend.modern import compile as modern

    source = Path(__file__).resolve().parents[1] / "frontends" / "modern" / "fixtures" / f"{fixture}.mod"
    return _procedure(masm.text(modern.assembled(driver.parsed(source), entry="main")), name)


def test_a_dword_read_only_as_offset_and_selector_is_one_far_load() -> None:
    """modern sum split each far pointer through the stack.

    `mov ecx,[bp+6]; mov bx,cx; push ecx; pop cx; pop es` -- five
    instructions and two 66h prefixes for what `les bx,[bp+6]` does.
    """
    function = _modern("sum", "_sum")
    assert function.count("les ") == 2
    assert "pop es" not in function


def test_a_far_load_selector_takes_the_segment_register_allocation_gives_it() -> None:
    """C sum_three pinned every far load's selector to ES.

    `les bx,[bp+10]; mov fs,es` where `lfs` loads FS directly; all three
    descriptors went through ES.
    """
    from pathlib import Path

    from qbopt.cfront import compile as cfront

    source = Path(__file__).resolve().parents[1] / "bench" / "parity" / "sum_three.c"
    function = _procedure(cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True), "_sum_three")
    assert "lfs " in function and "lgs " in function
    assert "mov fs, es" not in function and "mov gs, es" not in function


def test_a_loop_base_plan_prices_the_index_it_spills() -> None:
    """modern sum_three kept its three bases and spilled the shared index.

    The plan's own index spill was not priced, so it looked free: the loop
    read and wrote the index's slot on every trip instead of reloading two
    bases. Its unfolded `add di,bx` was unpriced too, one more per trip.
    """
    function = _modern("sum_three", "_sum_three")
    loop = function[function.index("L0_3:") : function.index("L0_5:")]
    adds = [line.split(None, 1)[1] for line in loop.splitlines() if line.strip().startswith("add")]
    assert sorted(adds)[-1] == "bx, 2" and len(adds) == 4
    assert all(one.startswith("ax, word ptr") for one in adds[:-1])
