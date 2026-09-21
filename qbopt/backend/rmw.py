"""Select exact read-modify-write chains before register allocation.

An ordinary compound update initially reaches LIR as ``mov old,[cell]``, a
binary operation, and ``mov [cell],result``.  x86's two-address form can write
the cell directly.  C's integer promotions make the byte case longer: for an
expression such as ``cell |= (unsigned char)mask`` the same update includes
two zero extends and a truncation.  Both are instruction-selection questions,
and both must be answered before their dead temporaries compete for registers.

This is deliberately part of lowering, not an LIR optimization pass.  MIR
retains the language-visible promotion sequence; lowering recognizes a legal
target form without teaching a MIR pass about registers or encodings.
"""

from collections import Counter
from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir


def selected(insns: tuple[lir.Insn, ...], users: Counter[int]) -> tuple[lir.Insn, ...]:
    """Select private load, integer update, and store chains as one RMW.

    The folded sequence has no result value, so the loaded value and result
    must be private to it.  Volatile accesses retain their explicit ordering.
    The promoted-byte form is deliberately narrower: it may not cross another
    memory access because its source-language volatile provenance is not
    retained by every promotion follower.
    """
    byte = tuple(_fold_byte(insns, users))
    return tuple(_fold_integer(byte, users))


_MEMORY_BINARY = frozenset({"add", "sub", "and", "or", "xor"})
_COMMUTATIVE = frozenset({"add", "and", "or", "xor"})


def _fold_integer(insns: tuple[lir.Insn, ...], users: Counter[int]) -> list[lir.Insn]:
    """Select ``load; op; store-same-cell`` as a memory-destination operation."""
    definitions = {value: (index, one) for index, one in enumerate(insns) for value in one.defines}
    replaced: dict[int, lir.Insn] = {}
    erased: set[int] = set()
    for store_at, store in enumerate(insns):
        found = _integer_chain(insns, store_at, definitions, users)
        if found is None:
            continue
        load_at, operation_at, cell, source, name = found
        # Moving the cell's read to the operation may cross only computations
        # without observable effects.  A nonvolatile source load is permitted:
        # two reads commute, even when they name the same bytes.  The operation
        # itself may move down only across zero-byte anchors, so its flags and
        # the update's store remain at the same observable point.
        if any(not _preparation(one) for one in insns[load_at + 1 : operation_at]):
            continue
        if any(not _anchor(one) for one in insns[operation_at + 1 : store_at]):
            continue

        what = ir.Semantics(ir.Operation.BINARY, name, (cell,), (cell, source))
        values = tuple(value.value for operand in (cell, source) for value in ir.values(operand))
        replaced[store_at] = replace(
            store,
            what=what,
            uses=tuple(dict.fromkeys(values)),
            defines=(),
            widths=(),
        )
        erased.update((load_at, operation_at))
    return _rewritten(insns, replaced, erased)


def _integer_chain(
    insns: tuple[lir.Insn, ...],
    store_at: int,
    definitions: dict[int, tuple[int, lir.Insn]],
    users: Counter[int],
) -> tuple[int, int, ir.Mem, ir.Held | ir.Imm, str] | None:
    store = insns[store_at]
    match store.what:
        case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Mem() as cell,), (ir.Held() as result,)):
            pass
        case _:
            return None
    if not _plain(store) or _volatile(store) or users[result.value] != 1:
        return None

    operation_definition = definitions.get(result.value)
    if operation_definition is None:
        return None
    operation_at, operation = operation_definition
    if operation_at >= store_at or not _plain(operation) or _volatile(operation):
        return None
    match operation.what:
        case ir.Semantics(
            ir.Operation.BINARY,
            str(name),
            (ir.Held() as made,),
            (ir.Held() as left, (ir.Held() | ir.Imm()) as right),
        ):
            pass
        case _:
            return None
    if (
        name not in _MEMORY_BINARY
        or made != result
        or operation.defines != (result.value,)
        or operation.symbol is True
        or cell.width not in (1, 2, 4)
        or cell.width != result.width
    ):
        return None

    candidates = ((left, right),)
    if name in _COMMUTATIVE and isinstance(right, ir.Held):
        candidates += ((right, left),)
    for old, source in candidates:
        if source.width != cell.width or users[old.value] != 1:
            continue
        load_definition = definitions.get(old.value)
        if load_definition is None:
            continue
        load_at, load = load_definition
        if load_at >= operation_at or not _plain(load) or _volatile(load):
            continue
        match load.what:
            case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held() as loaded,), (ir.Mem() as loaded_cell,)):
                if (
                    loaded == old
                    and load.defines == (old.value,)
                    and loaded_cell == cell
                    and loaded_cell.width == old.width == cell.width
                ):
                    return load_at, operation_at, cell, source, name
    return None


def _fold_byte(insns: tuple[lir.Insn, ...], users: Counter[int]) -> list[lir.Insn]:
    definitions = {value: (index, one) for index, one in enumerate(insns) for value in one.defines}
    replaced: dict[int, lir.Insn] = {}
    erased: set[int] = set()
    for store_at, store in enumerate(insns):
        found = _chain(insns, store_at, definitions, users)
        if found is None:
            continue
        load_at, mask, erase = found
        # No read, write, branch, call, trap-like opaque operation or source
        # ownership boundary may intervene.  Pure integer preparation of the
        # mask is allowed; it remains and produces the byte source register.
        if any(
            not _pure(one) for index, one in enumerate(insns[load_at + 1 : store_at], load_at + 1) if index not in erase
        ):
            continue
        assert store.what is not None
        cell = store.what.dests[0]
        assert isinstance(cell, ir.Mem)
        what = ir.Semantics(ir.Operation.BINARY, "or", (cell,), (cell, mask))
        values = tuple(value.value for value in ir.values(cell))
        replaced[store_at] = replace(
            store,
            what=what,
            uses=tuple(dict.fromkeys((*values, mask.value))),
            defines=(),
            widths=(),
        )
        erased.update(erase)
        erased.add(load_at)
    return _rewritten(insns, replaced, erased)


def _chain(
    insns: tuple[lir.Insn, ...],
    store_at: int,
    definitions: dict[int, tuple[int, lir.Insn]],
    users: Counter[int],
) -> tuple[int, ir.Held, set[int]] | None:
    store = insns[store_at]
    match store.what:
        case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Mem() as cell,), (ir.Held() as narrowed,)):
            pass
        case _:
            return None
    if cell.width != narrowed.width == 1 or not _plain(store):
        return None
    narrowed_definition = definitions.get(narrowed.value)
    if narrowed_definition is None or users[narrowed.value] != 1:
        return None
    narrowed_at, narrowed_from = narrowed_definition
    match narrowed_from.what:
        case ir.Semantics(ir.Operation.EXTEND, "movzx", (ir.Held() as narrowed_result,), (ir.Held() as joined,)):
            # The source-side byte view is deliberately not a distinct SSA
            # value.  ``movzx v15:word, v14:byte`` followed by ``mov [m],
            # v15:byte`` is the C truncation after integer promotion.
            if (
                narrowed_result.value != narrowed.value
                or narrowed_result.width != 2
                or narrowed.width != 1
                or joined.width != 1
                or not _plain(narrowed_from)
            ):
                return None
        case _:
            return None
    joined_definition = definitions.get(joined.value)
    if joined_definition is None or users[joined.value] != 1:
        return None
    joined_at, joined_from = joined_definition
    match joined_from.what:
        case ir.Semantics(
            ir.Operation.BINARY, "or", (ir.Held() as joined_result,), (ir.Held() as left, ir.Held() as right)
        ):
            if (
                joined_result.value != joined.value
                or joined_result.width != 2
                or joined.width != 1
                or left.width != right.width == 2
                or not _plain(joined_from)
            ):
                return None
        case _:
            return None
    candidates = ((_byte_load(definitions, users, left), right), (_byte_load(definitions, users, right), left))
    for loaded, mask_wide in candidates:
        if loaded is None:
            continue
        load_at, loaded_cell = loaded
        mask_definition = definitions.get(mask_wide.value)
        if mask_definition is None or users[mask_wide.value] != 1:
            continue
        mask_at, mask_from = mask_definition
        match mask_from.what:
            case ir.Semantics(ir.Operation.EXTEND, "movzx", (ir.Held() as mask_result,), (ir.Held() as mask,)):
                if mask_result != mask_wide or mask.width != 1 or not _plain(mask_from):
                    continue
            case _:
                continue
        if loaded_cell != cell:
            continue
        # The chain values are private.  Do not erase the mask's own
        # definition: it is the byte source of the final OR.
        return load_at, mask, {narrowed_at, joined_at, mask_at}
    return None


def _byte_load(
    definitions: dict[int, tuple[int, lir.Insn]], users: Counter[int], value: ir.Held
) -> tuple[int, ir.Mem] | None:
    definition = definitions.get(value.value)
    if definition is None or users[value.value] != 1:
        return None
    at, one = definition
    match one.what:
        case ir.Semantics(ir.Operation.EXTEND, "movzx", (ir.Held() as result,), (ir.Mem() as cell,)):
            if result == value and value.width == 2 and cell.width == 1 and _plain(one):
                return at, cell
    return None


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


def _volatile(one: lir.Insn) -> bool:
    return bool(getattr(one.op, "volatile", False))


def _anchor(one: lir.Insn) -> bool:
    return (
        one.what is not None
        and one.what.op is ir.Operation.NOTHING
        and not one.defines
        and not one.uses
        and _plain(one)
    )


def _preparation(one: lir.Insn) -> bool:
    """Whether the destination read may move across one source computation."""
    if _anchor(one):
        return True
    if not _plain(one) or one.what is None or _volatile(one):
        return False
    what = one.what
    if what.op not in (
        ir.Operation.MOVE,
        ir.Operation.ADDRESS,
        ir.Operation.BINARY,
        ir.Operation.UNARY,
        ir.Operation.EXTEND,
        ir.Operation.MULTIPLY,
    ):
        return False
    if any(isinstance(operand, (ir.Mem, ir.Reg)) for operand in what.dests):
        return False
    memory_sources = [operand for operand in what.sources if isinstance(operand, ir.Mem)]
    return not memory_sources or (what.op is ir.Operation.MOVE and len(memory_sources) == 1)


def _rewritten(insns: tuple[lir.Insn, ...], replaced: dict[int, lir.Insn], erased: set[int]) -> list[lir.Insn]:
    out = []
    for index, one in enumerate(insns):
        if index in replaced:
            out.append(replaced[index])
        elif index in erased:
            # Retain source ownership and anchors while removing the virtual
            # ranges before allocation.  lir.anchor intentionally keeps
            # definitions for post-allocation cleanup, so clear them here.
            out.append(replace(lir.anchor(one), defines=(), uses=(), widths=()))
        else:
            out.append(one)
    return out


def _pure(one: lir.Insn) -> bool:
    if not _plain(one) or one.what is None:
        return False
    # The replacement moves its selected memory read to the original store.
    # Even an intervening read is not necessarily ignorable: it may be a
    # source-language volatile access, whose provenance followers do not
    # retain.  Integer-only mask preparation is the deliberately narrow safe
    # region.
    return not any(isinstance(operand, ir.Mem) for operand in (*one.what.dests, *one.what.sources))
