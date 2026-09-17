from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.optimize import edges
from qbopt.analysis import liveness


def expanded(body: mir.MirBody) -> mir.MirBody:
    switches = [block for block in body.blocks if block.ops and block.ops[-1].kind is mir.Kind.SWITCH]
    if not switches:
        if any(op.kind is mir.Kind.SWITCH for block in body.blocks for op in block.ops):
            raise ValueError("switch must terminate its block")
        return body
    if any(op.kind is mir.Kind.SWITCH for block in body.blocks for op in block.ops[:-1]):
        raise ValueError("switch must terminate its block")
    live = liveness.live(body)
    labels = {block.at for block in body.blocks}
    for block in switches:
        op = block.ops[-1]
        if (
            len(op.args) != 1
            or not isinstance(op.args[0], (mir.Held, mir.Const))
            or op.args[0].width not in (1, 2, 4)
            or op.defines
            or op.results
            or op.loads
            or op.stores
            or op.merges
            or op.barrier
            or op.stack is not None
            or op.floating is not None
            or op.target not in labels
            or any(target not in labels for _, target in op.cases)
            or set(block.succ) != {op.target, *(target for _, target in op.cases)}
        ):
            raise ValueError("invalid semantic switch")
        mask = (1 << (8 * op.args[0].width)) - 1
        normalized = [value & mask for value, _ in op.cases]
        if len(set(normalized)) != len(normalized):
            raise ValueError("duplicate switch case value")
        if (
            isinstance(op.args[0], mir.Held)
            and any(target != op.target for _, target in op.cases)
            and any(value.flags for value in live.live_out[block.at])
        ):
            raise ValueError("switch expansion crosses a live condition")
    serial = max((value.id for value in ssa.values(body)), default=0)
    label = edges.fresh(body)
    replacements: dict[int, list[mir.MirBlock]] = {}
    incoming: dict[tuple[int, int], list[int]] = {}
    for block in switches:
        op = block.ops[-1]
        op = replace(op, cases=tuple((number, target) for number, target in op.cases if target != op.target))
        selector = op.args[0]
        assert isinstance(selector, (mir.Held, mir.Const)) and op.target is not None
        if not op.cases or isinstance(selector, mir.Const):
            target = op.target
            if isinstance(selector, mir.Const):
                mask = (1 << (8 * selector.width)) - 1
                target = next((target for number, target in op.cases if number & mask == selector.n & mask), target)
            jump = replace(op, kind=mir.Kind.JUMP, target=target, args=(), uses=(), cases=(), name="", raised=None)
            replacements[block.at] = [replace(block, ops=(*block.ops[:-1], jump), succ=(target,))]
            for successor in block.succ:
                incoming[(successor, block.at)] = [block.at] if successor == target else []
            continue
        chain = [block.at, *range(label, label + len(op.cases) - 1)]
        label += len(op.cases) - 1
        rebuilt = []
        for index, (number, target) in enumerate(op.cases):
            at = chain[index]
            fallback = chain[index + 1] if index + 1 < len(chain) else op.target
            serial += 1
            condition = mir.Value(serial, op.at, flags=True, variable=serial, version=1)
            uses = (selector.value,) if isinstance(selector, mir.Held) else ()
            compare = mir.Op(
                op.at,
                ir.Operation.COMPARE,
                "cmp",
                (condition,),
                uses,
                kind=mir.Kind.SUB,
                args=(selector, mir.Const(number & ((1 << (8 * selector.width)) - 1), selector.width)),
                symbol=False,
            )
            branch = mir.Op(
                op.at,
                ir.Operation.BRANCH,
                "",
                (),
                (condition,),
                kind=mir.Kind.BRANCH,
                test=mir.Kind.EQ,
                target=target,
                symbol=False,
            )
            if index == 0:
                compare = replace(
                    compare,
                    absorbed=op.absorbed,
                    id=op.id,
                    source_backed=op.source_backed,
                )
            rebuilt.append(
                mir.MirBlock(
                    at,
                    block.phis if index == 0 else (),
                    (*(block.ops[:-1] if index == 0 else ()), compare, branch),
                    tuple(dict.fromkeys((target, fallback))),
                )
            )
            incoming.setdefault((target, block.at), []).append(at)
            if index + 1 == len(chain):
                incoming.setdefault((fallback, block.at), []).append(at)
        replacements[block.at] = rebuilt
    blocks = []
    for original in body.blocks:
        for block in replacements.get(original.at, [original]):
            phis = []
            for phi in block.phis:
                sources = {
                    replacement: value
                    for source, value in phi.incoming.items()
                    for replacement in incoming.get((block.at, source), [source])
                }
                phis.append(replace(phi, incoming=sources))
            blocks.append(replace(block, phis=tuple(phis)))
    return replace(body, blocks=tuple(blocks), cloned=True)
