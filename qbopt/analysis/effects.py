"""Conservative memory effects not described by explicit MIR write ranges."""

from qbopt.model import mir

_TRAPS = frozenset({mir.Kind.FCHECK, mir.Kind.DIV, mir.Kind.REM, mir.Kind.DIVMOD})


def unmodeled_write(op: mir.Op) -> bool:
    """Missing call effects are unknown, not a proof that the call is readonly."""
    return (op.barrier and not op.memory_complete) or (op.kind is mir.Kind.CALL and not op.stores)


def exposes_memory(op: mir.Op, handles_errors: bool) -> bool:
    """Whether a trap here can reach an ON ERROR handler that reads memory.

    Without a handler a trappable error ends the program, and nothing reads
    memory after that.
    """
    return handles_errors and (op.floating is not None or op.kind in _TRAPS)
