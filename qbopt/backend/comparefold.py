"""Select one-use memory comparisons before register allocation.

MIR keeps a load and its comparison as separate language operations.  x86 can
compare the cell directly.  Selecting that form only after allocation makes a
dead temporary compete with values that really must survive the branch, which
can turn a removed load into an unrelated spill.  This is target lowering,
not an LIR optimization tier: the memory read stays at the same observable
point and only the legal machine operand is chosen.
"""

from collections import Counter
from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir


def selected(
    insns: tuple[lir.Insn, ...], users: Counter[int], exposed: set[int] | frozenset[int]
) -> tuple[lir.Insn, ...]:
    """Fold a private load followed by its sole comparison into a memory comparison.

    Zero-byte anchors may separate the two source operations.  No emitted
    instruction is crossed, so volatile order, faults and flags are unchanged.
    The other comparison operand must not already be memory: x86 has no
    memory-to-memory comparison form.  A widening load may fold only for an
    equality test against zero: narrowing that comparison preserves ZF, while
    a signed condition could observe the narrow cell's top bit through SF.
    """
    out = list(insns)
    for load_at, load in enumerate(insns):
        extension = False
        match load.what:
            case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(value, width),), (ir.Mem() as cell,)):
                pass
            case ir.Semantics(
                ir.Operation.EXTEND,
                "movsx" | "movzx",
                (ir.Held(value, width),),
                (ir.Mem() as cell,),
            ) if cell.width < width:
                extension = True
            case _:
                continue
        if (
            (cell.width != width and not extension)
            or load.defines != (value,)
            or users[value] != 1
            or value in exposed
            or not _plain(load)
        ):
            continue

        compare_at = load_at + 1
        while compare_at < len(insns) and _anchor(insns[compare_at]):
            compare_at += 1
        if compare_at == len(insns):
            continue
        compare = insns[compare_at]
        match compare.what:
            case ir.Semantics(ir.Operation.COMPARE, "cmp", (), sources):
                pass
            case _:
                continue
        if len(sources) != 2 or not _plain(compare) or compare.defines:
            continue
        positions = [index for index, source in enumerate(sources) if source == ir.Held(value, width)]
        if len(positions) != 1:
            continue
        position = positions[0]
        other = sources[1 - position]
        if isinstance(other, ir.Mem) or getattr(other, "width", None) != width:
            continue
        if extension:
            if not isinstance(other, ir.Imm) or other.value != 0 or other.address is not None:
                continue
            branch_at = compare_at + 1
            while branch_at < len(insns) and _anchor(insns[branch_at]):
                branch_at += 1
            if branch_at == len(insns):
                continue
            branch = insns[branch_at]
            if (
                branch.what is None
                or branch.what.op is not ir.Operation.BRANCH
                or branch.what.name not in {"je", "jne"}
            ):
                continue
            other = ir.Imm(0, cell.width)
        # A displacement and immediate can each own a relocation.  LIR's
        # ownership flag names one source operation, so retain the unfused
        # pair rather than silently dropping either fixup.
        if load.symbol is True and compare.symbol is True:
            continue

        folded_sources = list(sources)
        folded_sources[position] = cell
        folded_sources[1 - position] = other
        address_uses = tuple(held.value for held in ir.values(cell))
        compare_uses = tuple(one for one in compare.uses if one != value)
        out[compare_at] = replace(
            compare,
            what=replace(compare.what, sources=tuple(folded_sources)),
            uses=tuple(dict.fromkeys((*address_uses, *compare_uses))),
            op=load.op if load.symbol is True else compare.op,
            symbol=True if load.symbol is True else compare.symbol,
        )
        # Keep source ownership and anchors while deleting the virtual range
        # before allocation.  This is the same ownership transaction used by
        # pre-allocation RMW selection.
        out[load_at] = replace(lir.anchor(load), defines=(), uses=(), widths=())
    return tuple(out)


def _plain(one: lir.Insn) -> bool:
    return not (
        one.clobbers
        or one.clobbers_high
        or one.requires
        or one.delivers
        or one.spread
        or one.group is not None
        or one.frame_adjust
        or one.spill_reload
        or one.spill_store
        or one.rematerialized
    )


def _anchor(one: lir.Insn) -> bool:
    return (
        one.what is not None
        and one.what.op is ir.Operation.NOTHING
        and not one.defines
        and not one.uses
        and _plain(one)
    )
