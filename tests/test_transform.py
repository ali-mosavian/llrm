"""
qbopt/transform.py's own gate.

What is checked here is mostly the *order* the transforms run in and what
each one had to be right about, because widening was written wrong twice and
neither time could the host suite see it.
"""

from iced_x86 import Register

from qbopt import ir
from qbopt import wide
from qbopt import transform


def test_widening_is_on_and_runs_after_the_memory_passes() -> None:
    """Order, not preference. A widened op lies about how much it reads.

    `mov eax,[x]` keeps the low half's own `loads` -- two bytes at [x] --
    while the instruction reads four, so avail.py asked whether [x+2] had
    been written and was told nothing had touched it, and forwarded a stale
    high half. Running widening after the passes that reason about memory
    means none of them ever sees the mismatch.

    The reverse order was deliberate and its reason was real: folding a pair
    retires the carry between its halves, which makes a later reload of the
    same cell visible as redundant rather than as the high half's own read.
    Worth having, and not at that price.
    """
    import inspect

    signature = inspect.signature(transform.applied)
    assert signature.parameters["widen"].default is True
    assert signature.parameters["place"].default is False, "placement moves code and buys nothing yet"
    assert signature.parameters["drop_loads"].default is True
    assert signature.parameters["drop_stores"].default is True

    source = inspect.getsource(transform.applied)
    assert source.index("body = widened(body)") > source.index("without_dead_stores("), (
        "widening has to run after the passes that read an op's loads and stores"
    )


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
