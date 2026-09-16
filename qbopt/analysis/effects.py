"""Conservative memory effects not described by explicit MIR write ranges."""

from qbopt.model import mir

_TRAPS = frozenset({mir.Kind.FCHECK, mir.Kind.DIV, mir.Kind.REM, mir.Kind.DIVMOD})


def unmodeled_write(op: mir.Op) -> bool:
    """Whether an operation may write beyond its explicit MIR store ranges.

    An empty footprint means ``readonly`` only when the raise marked the
    footprint complete.  Without that bit an unknown call and a proven pure
    call both spell ``stores=()``, and treating either spelling as the other
    is unsound in one direction and needlessly pessimistic in the other.
    """
    return (op.barrier or op.kind is mir.Kind.CALL) and not op.memory_complete


def unmodeled_read(op: mir.Op) -> bool:
    """Whether an operation may read beyond its explicit MIR load ranges."""
    return (op.barrier or op.kind is mir.Kind.CALL) and not op.memory_complete


def exposes_memory(op: mir.Op, handles_errors: bool) -> bool:
    """Whether a trap here can reach an ON ERROR handler that reads memory.

    Without a handler a trappable error ends the program, and nothing reads
    memory after that.
    """
    return handles_errors and (op.floating is not None or op.kind in _TRAPS)
