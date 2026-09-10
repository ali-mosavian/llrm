"""Apply established runtime control contracts before constructing value SSA."""

from dataclasses import replace

from qbopt.abi import runtime
from qbopt.frontend.blocks import Block, Ends


def terminal_edges(blocks: list[Block], contracts: dict[int, runtime.Contract]) -> list[Block]:
    """A terminal call at a block boundary has no normal-return successor.

    Interior calls need a separate byte-owning block split; this changes no
    instruction spans, and never treats an unestablished contract as proof.
    """
    result = []
    for block in blocks:
        contract = contracts.get(block.insns[-1].at) if block.insns else None
        if contract is not None and contract.established and contract.control is runtime.Control.NEVER:
            block = replace(block, ends=Ends.LEAVES, succ=())
        result.append(block)
    return result
