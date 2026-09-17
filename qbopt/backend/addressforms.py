from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import mir
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def offsets(body: mir.MirBody) -> dict[int, tuple[ir.Held, int]]:
    result = {}
    for block in body.blocks:
        for op in block.ops:
            if op.loads or op.stores or op.barrier:
                continue
            match op.kind, op.args, op.results:
                case mir.Kind.COPY, (mir.Held(width=2) as source,), (mir.Held(value=dest, width=2),):
                    result[dest.id] = ir.Held(source.value.id, 2), 0
                case mir.Kind.ADD, (mir.Held(width=2) as source, mir.Const(n=amount, width=2)), (
                    mir.Held(value=dest, width=2),
                ):
                    result[dest.id] = ir.Held(source.value.id, 2), amount
                case _:
                    pass
    return result


def selected(what: ir.Semantics | None, forms: dict[int, tuple[ir.Held, int]]) -> ir.Semantics | None:
    if what is None:
        return None

    def operand(arg: object) -> object:
        if not isinstance(arg, ir.Mem) or arg.base is None or arg.base.width != 2 or arg.index is not None:
            return arg
        if arg.addr is None or arg.addr.space is not Space.FAR:
            return arg
        base, offset = arg.base, 0
        seen = set()
        while base.value in forms and base.value not in seen:
            seen.add(base.value)
            base, step = forms[base.value]
            offset += step
        if base.value in seen:
            return arg
        displacement = (arg.offset + offset + 32768) % 65536 - 32768
        return (
            replace(
                arg,
                base=base,
                offset=displacement,
                disp_width=2,
                addr=replace(arg.addr, disp=(arg.addr.disp + offset + 32768) % 65536 - 32768),
            )
            if seen
            else arg
        )

    return replace(what, dests=tuple(map(operand, what.dests)), sources=tuple(map(operand, what.sources)))


# Scales 32-bit addressing encodes; 16-bit `[bx+si]` has none but one.
_SCALES = {4: (0, 1, 2, 3), 2: (0,)}
type IndexedBase = ir.Held | ir.Address
type IndexedForm = tuple[IndexedBase, ir.Held, int]
type FoldedForm = IndexedForm | ir.Address


def indexed(body: mir.MirBody, exposed: set[int]) -> tuple[dict[int, FoldedForm], frozenset[int]]:
    """Based addresses `b + (c << k)` read only by cells, and what computes them.

    The address becomes the cell's `[base+index*scale]` and the add and
    shift that computed it become nothing. Only where no flag they set is
    read and nothing but an encodable cell's base reads the address.  This
    applies equally to far pointers and near pointers into local or global
    objects; the address width below decides whether a scale is legal.
    """
    made = {value.id: op for block in body.blocks for op in block.ops for value in op.defines}
    frame_bases = {
        result.value.id: ir.Address(
            Addr(Space.FRAME, source.offset),
            Register.BP,
            offset=source.offset,
            disp_width=1 if -128 <= source.offset <= 127 else 2,
        )
        for op in made.values()
        if op.kind is mir.Kind.ADDRESS
        and not (op.loads or op.stores or op.merges or op.barrier)
        and len(op.args) == len(op.results) == 1
        and isinstance((source := op.args[0]), mir.FrameAddress)
        and source.width == 2
        and isinstance((result := op.results[0]), mir.Held)
        and result.width == 2
    }
    # Strength reduction may choose a convenient fixed address and derive
    # several elements from it (for example ``&x[4] - 16``).  They are all
    # the same encodable BP displacement.  Discover the complete pure
    # constant chain before use classification, so an intermediate address
    # is not mistaken for a value that needs a register merely because a
    # second address expression reads it.
    fixed_candidates = dict(frame_bases)
    while True:
        before = len(fixed_candidates)
        for op in made.values():
            if (
                op.loads
                or op.stores
                or op.merges
                or op.barrier
                or len(op.results) != 1
                or not isinstance(op.results[0], mir.Held)
                or op.results[0].width != 2
            ):
                continue
            source = None
            amount = 0
            if (
                op.kind is mir.Kind.COPY
                and len(op.args) == 1
                and isinstance(op.args[0], mir.Held)
                and op.args[0].width == 2
            ):
                source = op.args[0]
            elif op.kind is mir.Kind.ADD and len(op.args) == 2:
                held = [one for one in op.args if isinstance(one, mir.Held) and one.width == 2]
                constants = [one for one in op.args if isinstance(one, mir.Const) and one.width == 2]
                if len(held) == len(constants) == 1:
                    source, amount = held[0], constants[0].n
            if source is None or source.value.id not in fixed_candidates:
                continue
            fixed = fixed_candidates[source.value.id]
            displacement = (fixed.offset + amount + 32768) % 65536 - 32768
            fixed_candidates[op.results[0].value.id] = replace(
                fixed,
                addr=replace(fixed.addr, disp=displacement) if fixed.addr is not None else None,
                offset=displacement,
                disp_width=1 if -128 <= displacement <= 127 else 2,
            )
        if len(fixed_candidates) == before:
            break
    bases: dict[int, int] = {}
    other: dict[int, int] = {}
    for block in body.blocks:
        for phi in block.phis:
            for value in phi.incoming.values():
                other[value.id] = other.get(value.id, 0) + 1
        for op in block.ops:
            based = {
                one.ref.base.id
                for one in (*op.args, *op.results)
                if isinstance(one, mir.Cell)
                and one.ref.base is not None
                and one.ref.addr is not None
                and one.ref.where not in (Space.GROUP, Space.STACK)
            }
            held = [one.value.id for one in op.args if isinstance(one, mir.Held)]
            for value in op.uses:
                if value.id in based and value.id not in held:
                    bases[value.id] = bases.get(value.id, 0) + 1
                else:
                    other[value.id] = other.get(value.id, 0) + 1
    for value in exposed:
        other[value] = other.get(value, 0) + 1

    def plain(op: mir.Op, kind: mir.Kind) -> bool:
        return (
            op.kind is kind
            and not (op.loads or op.stores or op.merges or op.barrier)
            and len(op.results) == 1
            and isinstance(op.results[0], mir.Held)
            and not any(one.flags and (one.id in other or one.id in bases) for one in op.defines)
        )

    # A candidate whose arithmetic flags are observable is a value
    # computation, not merely an address spelling.  Admit derived fixed
    # addresses in dependency order only when deleting their operation is
    # legal; the use classification above is what makes that answer exact.
    fixed_frames = dict(frame_bases)
    while True:
        before = len(fixed_frames)
        for op in made.values():
            if (
                op.kind not in (mir.Kind.COPY, mir.Kind.ADD)
                or not plain(op, op.kind)
                or len(op.results) != 1
                or not isinstance(op.results[0], mir.Held)
                or op.results[0].value.id not in fixed_candidates
                or not any(isinstance(one, mir.Held) and one.value.id in fixed_frames for one in op.args)
            ):
                continue
            fixed_frames[op.results[0].value.id] = fixed_candidates[op.results[0].value.id]
        if len(fixed_frames) == before:
            break

    forms: dict[int, FoldedForm] = {}
    folded: set[int] = set()
    for value, fixed in fixed_frames.items():
        if value not in bases:
            continue
        # A frame address consumed directly as a cell base is already the
        # cell's BP displacement. Whether its ADDRESS operation is dead is
        # settled after all dependent address expressions have been folded.
        forms[value] = fixed
    for block in body.blocks:
        for op in block.ops:
            if not plain(op, mir.Kind.ADD) or len(op.args) != 2:
                continue
            address = op.results[0]
            if address.value.id in other or address.value.id not in bases or address.width not in _SCALES:
                continue
            held = [one for one in op.args if isinstance(one, mir.Held) and one.width == address.width]
            constants = [one for one in op.args if isinstance(one, mir.Const) and one.width == address.width]
            if len(held) == len(constants) == 1 and (fixed := fixed_frames.get(address.value.id)) is not None:
                # `&local[0] + 8` is an address spelling, not a value worth
                # carrying.  Full unrolling exposes many of these with a
                # literal subscript; keep the 16-bit wrapping arithmetic and
                # put the result straight in the BP displacement.
                forms[address.value.id] = fixed
                folded.add(address.value.id)
                continue
            if not all(isinstance(one, mir.Held) and one.width == address.width for one in op.args):
                continue
            base, index = op.args
            fixed = fixed_frames.get(base.value.id)
            form = (fixed or ir.Held(base.value.id, base.width), ir.Held(index.value.id, index.width), 1)
            for base, index in (op.args, op.args[::-1]):
                shift = made.get(index.value.id)
                if (
                    shift is not None
                    and plain(shift, mir.Kind.SHL)
                    and len(shift.args) == 2
                    and isinstance(shift.args[0], mir.Held)
                    and shift.args[0].width == address.width
                    and isinstance(shift.args[1], mir.Const)
                    and shift.args[1].n in _SCALES[address.width]
                    and other.get(index.value.id, 0) == 1
                    and index.value.id not in bases
                ):
                    counter = shift.args[0]
                    fixed = fixed_frames.get(base.value.id)
                    form = (
                        fixed or ir.Held(base.value.id, base.width),
                        ir.Held(counter.value.id, counter.width),
                        1 << shift.args[1].n,
                    )
                    folded.add(index.value.id)
                    break
            forms[address.value.id] = form
            folded.add(address.value.id)

    phi_reads = {value.id for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    # Prove deletion backwards from actual folded memory operands.  Being a
    # recognizable fixed address is insufficient: its defining arithmetic
    # may remain as an ordinary value computation.  The old forward test
    # accepted any child in ``fixed_frames`` and deleted the parent LEA while
    # leaving ``add child,parent,constant`` behind with an undefined source.
    # A chain becomes dead only after every child operation that reads it is
    # itself in ``folded``; iterate because the proof runs from leaves to root.
    while True:
        before = len(folded)
        for value in fixed_frames:
            if value in folded or value in exposed or value in phi_reads:
                continue
            for block in body.blocks:
                for op in block.ops:
                    if not any(one.id == value for one in op.uses):
                        continue
                    based = {
                        one.ref.base.id
                        for one in (*op.args, *op.results)
                        if isinstance(one, mir.Cell) and one.ref.base is not None
                    }
                    held = any(isinstance(one, mir.Held) and one.value.id == value for one in op.args)
                    derived = held and any(
                        isinstance(result, mir.Held) and result.value.id in folded for result in op.results
                    )
                    if (held and not derived) or (not held and value not in based):
                        break
                else:
                    continue
                break
            else:
                folded.add(value)
        if len(folded) == before:
            break
    return forms, frozenset(folded)


def scaled(what: ir.Semantics | None, forms: dict[int, FoldedForm]) -> ir.Semantics | None:
    """`what` with every folded far address written as its cell's base and index."""
    if what is None or not forms:
        return what

    def operand(arg: object) -> object:
        if not isinstance(arg, ir.Mem) or arg.base is None or arg.base.value not in forms:
            return arg
        form = forms[arg.base.value]
        if isinstance(form, ir.Address):
            base, index, scale = form, None, 1
        else:
            base, index, scale = form
        if isinstance(base, ir.Address):
            if (
                base.addr is None
                or base.addr.space is not Space.FRAME
                or arg.addr is None
                or arg.addr.space is not Space.LITERAL
            ):
                return arg
            # A frame address is already BP plus a constant displacement.
            # Keep the dynamic byte offset as the word index and put the
            # constant directly in the memory operand: [bp+si+disp].  The
            # literal spelling says no relocation owns the displacement;
            # SS preserves the frame selector when the data model has DS != SS.
            displacement = (arg.addr.disp + base.offset + 32768) % 65536 - 32768
            if index is None:
                return replace(
                    arg,
                    addr=replace(base.addr, disp=displacement),
                    through=Register.NONE,
                    base=None,
                    index=None,
                    scale=1,
                )
            return replace(
                arg,
                addr=replace(arg.addr, space=Space.LITERAL, disp=displacement, segment=Register.SS),
                through=Register.BP,
                base=None,
                index=index,
                scale=scale,
            )
        return replace(arg, base=base, index=index, scale=scale, through=Register.NONE)

    return replace(what, dests=tuple(map(operand, what.dests)), sources=tuple(map(operand, what.sources)))
