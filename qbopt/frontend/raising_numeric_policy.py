"""Separate BASIC's explicit overflow observation from native integer arithmetic."""

from dataclasses import replace

from iced_x86 import Code

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model.floating import Exceptions
from qbopt.frontend.raising_calls import _discarded


def checkpoints(body: mir.MirBody) -> mir.MirBody:
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    _discarded(op)
                    if op.kind is mir.Kind.FCHECK
                    else replace(op, floating=replace(op.floating, exceptions=Exceptions.DEFERRED))
                    if op.floating is not None
                    else op
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )


def native(body):
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    _discarded(op) if isinstance((node := getattr(op, "node", None)), ir.Opaque)
                    and node.insn.insn.code == Code.INTO else op
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )
