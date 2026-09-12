"""A call destroys the selectors a routine it reaches may change.

With ES, FS and GS allocatable, a selector lives across calls. The clobber
mask named only the general registers, so a B$HARY call (which returns ES:BX)
left a selector live in ES, and a routine reaching user code left one in FS.
"""

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower


def _call(name: str) -> frozenset:
    op = mir.Op(0x10, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)
    return lower._clobbers(op, {0x10: name})


def test_a_routine_that_returns_es_destroys_es_and_keeps_fs() -> None:
    clobbered = _call("B$HARY")
    assert Register.ES in clobbered
    assert Register.FS not in clobbered and Register.GS not in clobbered


def test_a_routine_reaching_user_code_destroys_every_selector() -> None:
    assert {Register.ES, Register.FS, Register.GS} <= _call("B$LINA")


def test_an_unknown_call_destroys_every_selector() -> None:
    assert {Register.ES, Register.FS, Register.GS} <= _call("SOMESUB")
