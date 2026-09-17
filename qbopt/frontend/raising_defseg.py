"""DEF SEG as the store it is.

`DEF SEG = &HA000` is `mov ax,0A000h / push ax / call far B$DSEG`, and
B$DSEG is eleven bytes that move the pushed word into the runtime's own
`b$seg` -- the cell PEEK and POKE read their segment from. Left as a call,
the constant is invisible: every `mov es,[b$seg]` is an opaque load and the
POKE store addressed through it aliases every other reference in its loop,
which is 67% of qbdemo's run time.

Its contract says `writes = NONE` on the argument that b$seg is not
anything the caller can name. The caller does name it -- that is the
`mov es,[b$seg]` at every POKE -- so making the write explicit here is also
the honest version of that claim.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.objectfile import omf
from qbopt.objectfile import module

_CELL = "b$seg"
_NAME = "B$DSEG"


def _self_xor(op) -> bool:
    """`xor r,r`, which computes a constant and reads nothing.

    BC writes one after a call whose result it is discarding, and while it
    stands it reads a value only the call defines -- which is what stops the
    call being replaced. raising_array_bounds already interprets the idiom
    this way; this is the same fact where it unblocks something.
    """
    return (
        op.kind is mir.Kind.XOR
        and len(op.args) == 2
        and op.args[0] == op.args[1]
        and isinstance(op.args[0], mir.Held)
        and not op.loads
        and not op.stores
    )


def _zeroed(op):
    return replace(
        op,
        kind=mir.Kind.COPY,
        op=ir.Operation.MOVE,
        name="mov",
        args=(mir.Const(0, op.args[0].width),),
        uses=(),
        node=None,
        symbol=False,
        covers=op.covers or (op.at, op.at),
    )


def _pushed(op) -> "mir.Value | None":
    if op.kind is not mir.Kind.ARG or len(op.args) != 1:
        return None
    return op.args[0].value if isinstance(op.args[0], mir.Held) else None


def _fixup(found) -> int | None:
    """Where the object names b$seg with a 16-bit offset fixup.

    The store needs a relocation of its own and the call's is a ptr32 to a
    routine -- the wrong kind and the wrong target. Every PEEK and POKE in
    the program already carries the right one.
    """
    names = omf.externals(found.records)
    if _CELL not in names:
        return None
    which = names.index(_CELL)
    return next(
        (
            one.offset
            for one in omf.fixups(found.records)
            if one.seg == found.seg and one.target == "external" and one.index == which and one.loc == omf.LOC_OFF16
        ),
        None,
    )


def _candidate(op, block_ops, found, contracts, local, expected) -> "tuple | None":
    """(what this call sets b$seg to, the value pushed), or None."""
    if op.kind is not mir.Kind.CALL or found.calls.get(op.at) != _NAME or _NAME in local:
        return None
    contract = contracts.get(op.at)
    if (
        contract is None
        or not contract.established
        or contract.cleanup != expected.cleanup
        or contract.control is not runtime.Control.RETURNS
        or contract.enters_user_code
        or contract.raises_error
        or contract.error_handling
        or contract.clobbers != expected.clobbers
    ):
        return None
    where = block_ops.index(op)
    if where == 0:
        return None
    pushed = _pushed(block_ops[where - 1])
    if pushed is None:
        return None
    made = next(
        (
            one
            for one in block_ops[: where - 1]
            if one.kind is mir.Kind.COPY
            and len(one.args) == 1
            and isinstance(one.args[0], mir.Const)
            and one.defines == (pushed,)
        ),
        None,
    )
    # The literal where the push is one, and the pushed value otherwise.
    # What the rewrite is for is making the write to `b$seg` visible, and
    # `DEF SEG = <expression>` writes it exactly as `DEF SEG = &HA000` does;
    # requiring a constant left five of qbdemo's eleven as opaque calls,
    # each of which then ended every memory fact in its body.
    return (made.args[0] if made is not None else mir.Held(pushed, 2)), pushed


def raised(body: mir.MirBody, found, contracts, source: module.SourceMap | None = None) -> mir.MirBody:
    source = source or module.SourceMap.from_module(found)
    field = _fixup(found)
    if field is None:
        return body
    names = omf.externals(found.records)
    ref = mir.MemRef(addr=module.Addr(module.Space.EXTERNAL, 0, names.index(_CELL)), width=2)
    local = module.defines(found.records, found.seg)
    expected = runtime.contract(_NAME)

    wanted = {
        id(op): found_at
        for block in body.blocks
        for op in block.ops
        if (found_at := _candidate(op, list(block.ops), found, contracts, local, expected)) is not None
    }
    if not wanted:
        return body

    # Only the discards standing in the way of one of these, not every
    # `xor r,r` in the body -- `xor ax,ax` is two bytes and `mov ax,0` is
    # three, so rewriting them wholesale trades size for nothing.
    clobbered = {value for block in body.blocks for op in block.ops if id(op) in wanted for value in op.defines}
    # What the call leaves in ax is the word it was handed: rt/rtinit.asm is
    # `MOV AX,newseg` / `MOV [b$seg],AX`, so its one clobber is that value
    # and not an unknown. Anything reading it reads the pushed value, which
    # is still defined here -- so a reader is served by saying so rather than
    # by keeping the call, which is what blocked eight of qbdemo's eleven.
    swap = {
        one.id: pushed
        for block in body.blocks
        for op in block.ops
        if (got := wanted.get(id(op))) is not None
        for pushed in (got[1],)
        for one in op.defines
        if not one.flags
    }
    blocks = [
        replace(
            block, ops=tuple(_zeroed(op) if _self_xor(op) and set(op.uses) <= clobbered else op for op in block.ops)
        )
        for block in body.blocks
    ]

    read = {value for block in blocks for op in block.ops for value in op.uses}
    read.update(value for block in blocks for phi in block.phis for value in phi.incoming.values())

    out = []
    for block in blocks:
        ops: list = []
        for op in block.ops:
            got = wanted.get(id(op))
            value = got[0] if got is not None else None
            if value is not None and not any(one.flags and one in read for one in op.defines):
                gone = ops.pop()  # the push, whose word the callee popped
                # Both spans, as bytes: the push's and the call's own. With
                # node cleared nothing derives a length, and layout refuses a
                # body it cannot account for every byte of -- rightly, since
                # that is how it catches data BC put between instructions.
                span = ir.span(op.node) if op.node is not None else None
                pushed_span = ir.span(gone.node) if gone.node is not None else None
                if span is None or pushed_span is None:
                    ops.append(gone)
                    ops.append(op)
                    continue
                op = replace(
                    op,
                    kind=mir.Kind.STORE,
                    op=ir.Operation.MOVE,
                    name="mov",
                    args=(value,),
                    results=(mir.Cell(ref=ref),),
                    defines=(),
                    uses=() if isinstance(value, mir.Const) else (value.value,),
                    loads=(),
                    stores=(ref,),
                    merges={},
                    node=None,
                    raised=None,
                    symbol=True,
                    covers=(pushed_span[0], span[1]),
                )
                source.refs[op.id] = (field,)
            ops.append(op)
        out.append(replace(block, ops=tuple(ops)))
    body = replace(body, blocks=tuple(out))
    if not swap:
        return body
    from qbopt.analysis import ssa

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=tuple(
                    replace(phi, incoming={at: ssa.provider(one, swap) for at, one in phi.incoming.items()})
                    for phi in block.phis
                ),
                ops=tuple(ssa.substituted(op, swap) for op in block.ops),
            )
            for block in body.blocks
        ),
    )
