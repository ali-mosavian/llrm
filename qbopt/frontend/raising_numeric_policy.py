"""Separate BASIC's explicit overflow observation from native integer arithmetic."""

from dataclasses import replace

from iced_x86 import Code

from qbopt.frontend.raising_calls import _discarded
from qbopt.model import ir


def native(body):
    return replace(body, blocks=tuple(replace(block, ops=tuple(
        _discarded(op) if isinstance(op.node, ir.Opaque) and op.node.insn.insn.code == Code.INTO else op
        for op in block.ops)) for block in body.blocks))
