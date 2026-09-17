from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.optimize import edges
from qbopt.objectfile import module
from qbopt.frontend.blocks import Block
from qbopt.frontend import raising_words
from qbopt.frontend.blocks import dispatch_targets


def raised(body: mir.MirBody, found: module.Module, machine_blocks: list[Block]) -> mir.MirBody:
    if module.family(found.records) not in {"qb45", "pds71", "vbdos"}:
        return body
    if "B$OGTA" in module.defines(found.records, found.seg):
        return body
    if any(
        op.barrier or op.kind is mir.Kind.OPAQUE or any(ref.segment is not None for ref in (*op.loads, *op.stores))
        for block in body.blocks
        for op in block.ops
    ):
        return body
    exits = raising_words.leaving(body)
    candidate = ssa.pruned_phis(body, exits)
    readers = exits | {value for block in candidate.blocks for op in block.ops for value in op.uses}
    readers |= {value for block in candidate.blocks for phi in block.phis for value in phi.incoming.values()}
    instructions = {insn.at: insn for block in machine_blocks for insn in block.insns}
    labels = {block.at for block in candidate.blocks}
    serial = max((value.id for value in ssa.values(candidate)), default=0)
    label = edges.fresh(candidate)
    replacements: dict[int, tuple[int, int]] = {}
    blocks = []
    added = []
    for block in candidate.blocks:
        op = block.ops[-1] if block.ops else None
        if (
            op is None
            or op.kind is not mir.Kind.CALL
            or found.calls.get(op.at) != "B$OGTA"
            or op.at not in instructions
            or not op.args_known
            or len(op.args) != 1
            or not isinstance(op.args[0], (mir.Held, mir.Const))
            or op.args[0].width != 2
            or set(op.defines) & readers
        ):
            blocks.append(block)
            continue
        insn = instructions[op.at]
        targets = dispatch_targets(found, insn)
        if targets is None:
            blocks.append(block)
            continue
        default = insn.end + 1 + 2 * len(targets)
        if default not in labels or set(block.succ) != {default, *targets}:
            blocks.append(block)
            continue
        error, normal = label, label + 1
        label += 2
        serial += 1
        condition = mir.Value(serial, op.at, flags=True, variable=serial, version=1)
        selector = op.args[0]
        uses = (selector.value,) if isinstance(selector, mir.Held) else ()
        compare = mir.Op(
            op.at,
            ir.Operation.COMPARE,
            "cmp",
            (condition,),
            uses,
            kind=mir.Kind.SUB,
            args=(selector, mir.Const(255, 2)),
            symbol=False,
        )
        guard = mir.Op(
            op.at,
            ir.Operation.BRANCH,
            "",
            (),
            (condition,),
            kind=mir.Kind.BRANCH,
            test=mir.Kind.ABOVE,
            target=error,
            symbol=False,
        )
        dispatch = mir.Op(
            op.at,
            ir.Operation.JUMP,
            "",
            (),
            uses,
            kind=mir.Kind.SWITCH,
            args=(selector,),
            target=default,
            cases=tuple(enumerate(targets, 1)),
            symbol=False,
        )
        blocks.append(replace(block, ops=(*block.ops[:-1], compare, guard), succ=(error, normal)))
        added.extend((mir.MirBlock(error, (), (op,), block.succ), mir.MirBlock(normal, (), (dispatch,), block.succ)))
        replacements[block.at] = (error, normal)
    if not replacements:
        return body
    return replace(
        candidate,
        cloned=True,
        blocks=tuple(
            replace(
                block,
                phis=tuple(
                    replace(
                        phi,
                        incoming={
                            new: value
                            for source, value in phi.incoming.items()
                            for new in replacements.get(source, (source,))
                        },
                    )
                    for phi in block.phis
                ),
            )
            for block in (*blocks, *added)
        ),
    )
