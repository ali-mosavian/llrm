"""Select exact byte read-modify-write chains before register allocation.

C promotes ``unsigned char`` operands to ``int``.  For an expression such as
``cell |= (unsigned char)mask`` that can leave a byte load, two zero extends,
a word OR, a truncation and a byte store.  x86 has the exact operation as
``or byte ptr [cell],reg8``.  Folding it before allocation is important: the
otherwise-dead widened intermediates consume registers and can make an
unrelated hot-loop range split unplaceable.

This is deliberately part of lowering, not an LIR optimization pass.  MIR
retains the language-visible promotion sequence; lowering recognizes a legal
target form without teaching a MIR pass about registers or encodings.
"""

from collections import Counter
from dataclasses import replace

from qbopt.model import ir
from qbopt.model import lir


def selected(insns: tuple[lir.Insn, ...], users: Counter[int]) -> tuple[lir.Insn, ...]:
    """Select private ``movzx byte; or; mov byte`` chains as one byte RMW.

    The folded sequence has no result value: all its intermediate values must
    therefore be private to the chain.  The memory operand is both destination
    and first source, as x86's two-address OR requires.  Any non-pure work
    between the source load and the store is a barrier, even if it happens not
    to name the byte.  The replacement itself performs one byte load and one
    byte store, so it also preserves a volatile compound assignment's two
    accesses; no *other* memory access may be crossed.
    """
    return tuple(_fold_block(insns, users))


def _fold_block(insns: tuple[lir.Insn, ...], users: Counter[int]) -> list[lir.Insn]:
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
    narrowed_at, narrowed_from = definitions.get(narrowed.value, (None, None))
    if narrowed_from is None or users[narrowed.value] != 1:
        return None
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
    joined_at, joined_from = definitions.get(joined.value, (None, None))
    if joined_from is None or users[joined.value] != 1:
        return None
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
        mask_at, mask_from = definitions.get(mask_wide.value, (None, None))
        if mask_from is None or users[mask_wide.value] != 1:
            continue
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
    at, one = definitions.get(value.value, (None, None))
    if one is None or users[value.value] != 1:
        return None
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


def _pure(one: lir.Insn) -> bool:
    if not _plain(one) or one.what is None:
        return False
    # The replacement moves its selected memory read to the original store.
    # Even an intervening read is not necessarily ignorable: it may be a
    # source-language volatile access, whose provenance followers do not
    # retain.  Integer-only mask preparation is the deliberately narrow safe
    # region.
    return not any(isinstance(operand, ir.Mem) for operand in (*one.what.dests, *one.what.sources))
