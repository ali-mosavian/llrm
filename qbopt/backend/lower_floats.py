"""Refuse floating operations the backend cannot encode."""

from dataclasses import replace

from qbopt.frontend import raising_floats
from qbopt.model import mir


def _removed(op: mir.Op) -> bool:
    from qbopt.backend.lower import Unlowered

    if op.kind not in (mir.Kind.NOTHING, mir.Kind.FCHECK):
        return False
    if op.floating is not None or op.stack is not None:
        raise Unlowered("removed floating operation retains computation")
    return True


def checked(body: mir.MirBody) -> None:
    from qbopt.backend.lower import Unlowered

    repetitions = dict(body.repetitions)
    if len(repetitions) != len(body.repetitions) or any(
        body.block(at) is None or not 2 <= count <= len(body.block(at).ops) for at, count in body.repetitions
    ):
        raise Unlowered("invalid block repetition provenance")
    for block in body.blocks:
        for op in block.ops:
            if _removed(op) or op.floating is None:
                continue
            # Selection encodes the instruction's name and operands, never
            # `floating`; only the exception policy is not in the bytes.
            encoded = raising_floats.semantics(op)
            if encoded is None or replace(encoded, exceptions=op.floating.exceptions) != op.floating:
                raise Unlowered("floating semantics are not what the instruction encodes")
