"""
qbopt/transform.py's own gate.

What is checked here is mostly what is *not* on, and why: widening is
written and switched off because it was wrong in a way the host suite could
not see, and pinning the reason is the only thing that stops it being turned
back on by someone reading the checkbox.
"""

from iced_x86 import Register

from qbopt import ir
from qbopt import wide
from qbopt import transform


def test_widening_is_off_and_the_reason_is_not_a_preference() -> None:
    """`add ax,[x]` with `adc dx,[x+2]` is a long in dx:ax, and dx:ax is not eax.

    wide.widened() renames each half's operands to their 32-bit roots, which
    turns the pair into `add eax,[x]`: the carry lands in eax's high half and
    dx keeps whatever it held. suite/procs.bas printed 0x02040C10 where it
    wants 0x04080C10, on all twelve configurations, with the host suite
    green.

    What is missing is the step before the rename -- proving the pair is one
    value and choosing the register it becomes, which is lift.py's own pair
    machinery and what qbopt/pairs.py has started. Until that exists this
    stays off, and this test is what says so.
    """
    import inspect

    signature = inspect.signature(transform.applied)
    assert signature.parameters["widen"].default is False, (
        "on until the fuzz corpus has seen it -- matrix.py passed 12 of 12, "
        "and matrix.py passed for both whole-segment bugs too"
    )
    assert signature.parameters["place"].default is False, "placement moves code and buys nothing yet"
    assert signature.parameters["drop_loads"].default is True
    assert signature.parameters["drop_stores"].default is True


def test_the_rename_alone_is_what_was_unsound() -> None:
    """The concrete fact the chain and the restore exist to handle.

    Both halves of a dx:ax pair rename to their own roots -- ax to eax and
    dx to edx -- so a 32-bit operation on the pair is one register and dx is
    left holding what it held. That is not an argument against the rename;
    it is why a widened chain has to end in `push eax / pop ax / pop dx`.
    """
    assert ir.ROOT[Register.AX] is Register.EAX
    assert ir.ROOT[Register.DX] is Register.EDX
    # the two halves of BC's pair 0 root to different registers, which is
    # exactly why renaming one of them cannot express the whole long
    assert ir.ROOT[Register.AX] is not ir.ROOT[Register.DX]


def test_a_transform_accounts_for_every_byte_it_removes() -> None:
    """layout.py refuses a body it cannot cover, which is how it catches data
    BC put between the instructions. A deletion has to say what it took."""
    import inspect

    source = inspect.getsource(transform._absorb)
    assert "covers=" in source, "a deleted op's bytes must go to a survivor"
    assert "layout.selectable" in source, (
        "and only to one whose length comes from selection -- an op emitted "
        "verbatim is exactly as long as the bytes it copies"
    )
