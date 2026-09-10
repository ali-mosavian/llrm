"""Conservative memory effects not described by explicit MIR write ranges."""

from qbopt.model import mir


def unmodeled_write(op: mir.Op) -> bool:
    """Missing call effects are unknown, not a proof that the call is readonly."""
    return op.barrier or (op.kind is mir.Kind.CALL and not op.stores)
