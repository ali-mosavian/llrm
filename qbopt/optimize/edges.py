"""Place semantic operations on conditional edges, not on either arm."""

from dataclasses import replace

from qbopt.model import ir, mir


def explicit(block: mir.MirBlock, target: int) -> bool:
    return conditional(block, target) and block.ops[-1].target == target


def conditional(block: mir.MirBlock, target: int) -> bool:
    return (len(block.succ) == 2 and target in block.succ and bool(block.ops)
            and block.ops[-1].kind is mir.Kind.BRANCH and block.ops[-1].target in block.succ)


def fresh(body: mir.MirBody) -> int:
    return max((body.entry + 1) << 32, max(block.at for block in body.blocks)) + 1


def split(body: mir.MirBody, source: int, target: int, label: int,
          ops: tuple[mir.Op, ...]) -> mir.MirBody:
    parent = body.block(source)
    if parent is None or not conditional(parent, target) or body.block(label) is not None:
        raise ValueError("edge split does not identify a fresh conditional edge")
    jump = mir.Op(label, ir.Operation.JUMP, "", (), (), kind=mir.Kind.JUMP,
                  target=target, symbol=False)
    bridge = mir.MirBlock(label, (), (*ops, jump), (target,))
    blocks = []
    for block in body.blocks:
        if block.at == source:
            block = replace(block, succ=tuple(label if at == target else at for at in block.succ),
                            ops=(*block.ops[:-1], replace(block.ops[-1], target=label)
                                 if block.ops[-1].target == target else block.ops[-1]))
        if block.at == target:
            block = replace(block, phis=tuple(replace(phi, incoming={
                label if at == source else at: value for at, value in phi.incoming.items()
            }) for phi in block.phis))
        blocks.append(block)
    return replace(body, blocks=(*blocks, bridge))
