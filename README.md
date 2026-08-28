# qbopt

Make a `LONG` in QuickBASIC cost what an `INTEGER` costs.

BC compiles every 32-bit operation as two 16-bit ones, because it targets an
8086. On anything from a 386 up that is exactly twice the work, so longs are
avoided in the code that would most benefit from them -- and the avoidance,
not the arithmetic, is the thing worth removing.

    long / integer      BC      today    goal
    bitwise, additive   1.96     1.37     1.0
    multiply, divide    5.62     3.26     1.0

The "today" column is a runtime pass that lives in uGL and rewrites the
program's own code as it loads. It works, on QB 4.5, PDS 7.1 and VBDOS
across twelve switch combinations, and it is where every measurement here
comes from. It has also run out of room, which is why this repository
exists.

## Why a post-compilation pass

The runtime pass rewrites code in place, and cannot move it. That single
constraint bounds everything:

  * A region's widened form must fit in the bytes the original occupied.
    Anything larger is refused, however much better it would be.
  * Bytes a region saves become slack inside that region, jumped over. They
    cannot be lent to a neighbour, so running the pass twice buys nothing --
    measured: the second pass takes 0 regions and leaves *more* basic blocks
    than it found, because the jump over the slack is itself a terminator.
  * It costs about 16K of the program it is speeding up, competing with the
    46K BC has to compile in.
  * A call site has to be recognised by comparing a relocated segment and
    offset, and every test runs through DOSBox.

Working on the `.OBJ` between BC and LINK removes all of it. A call site
comes from a FIXUPP naming an EXTDEF, so "is this `B$CPI4`" is a lookup.
Relocations are records to edit rather than a hazard to avoid. Nothing
ships. Tests run on the host in milliseconds.

And code may be moved -- which had to be established rather than assumed,
because an intra-segment offset baked in without a relocation would be
invisible to a rewriter and would break silently. It is not: `ON k GOTO L1,
L2, L3` emits its three labels as three consecutive `offset16` fixups into
the module's own code segment. `tests/test_omf.py` asserts it.

## What is here

    qbopt/omf.py       read and write OMF; round trips byte-exact
    qbopt/declen.py    x86 instruction lengths, checked against ndisasm
    qbopt/lift.py      decoded code as 32-bit values, and back to bytes
    qbopt/price.py     cycle costs per part, 486 through Core
    fixtures/omf/      real BC output, one per configuration that differs
    tests/             hermetic, host-side
    docs/inherited-*   the plan and notes from the runtime pass

## What BC actually emits

Established by measurement, not from the manuals, and it is what any of this
has to handle. See `docs/inherited-plan.md` for the full matrix.

  * A long lives in a register pair, `ax:dx` or `cx:bx`, and every operation
    is done twice.
  * `*`, `\`, `MOD` and every comparison are calls into the runtime.
    Comparison pushes its left operand first; multiply and divide push it
    second. Uniform across all four configurations, opposite between the two
    routines -- and getting it backwards is silently a different answer.
  * `/G3` (VBDOS only) pushes a long argument as one dword; everything else
    pushes two words, high first.
  * A constant is split across the halves, each encoded as short as it fits:
    `sub ax,1234h` then `sbb dx,10h`.
  * The flags left behind are the *high half's*. One 32-bit operation leaves
    the whole result's: ZF differs on 18.7 per cent of cases, PF on 37.2, AF
    on 12.4. CF, SF, OF and the values never differ.

## Running

    python3 tests/test_omf.py
    python3 tests/test_lift.py
