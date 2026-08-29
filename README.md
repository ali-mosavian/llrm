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

## What it does

Measured over 110 objects of real BC output, across three compilers and twelve
switch combinations:

    1237 of 1332 regions taken, 24009 bytes -> 15321

Two kinds of rewrite. A **region** of long arithmetic becomes 386 code: one
32-bit operation where BC did two 16-bit ones, with the high half put back
through the stack in four bytes. A **runtime call** is absorbed: a long
comparison is fifteen or twenty-one bytes and eleven instructions behind a far
call, and becomes `mov eax,[a]` / `cmp eax,[b]` in nine. Multiply likewise.

Divide and remainder are absorbed as C compiles them, `mov eax,[a]` /
`mov ecx,[b]` / `cdq` / `idiv ecx`, with no test of the divisor because C makes
none. That is the one deliberate behaviour change: `x \ 0` and
`-2147483648 \ -1` fault, where BC's runtime raised error 11 for the first and
returned silently from the second. `B$MUI4` wraps on overflow, which is what
`imul` does, so multiply needed no such choice.

Everything is checked by building, linking and running the program: twelve
configurations, seven suite programs, compared line by line against a golden
authored from what the program means rather than captured from a compiler.

## What is here

    qbopt/omf.py       read and write OMF; round trips byte-exact
    qbopt/module.py    a module as the analysis sees it, addresses and all
    qbopt/declen.py    instructions, via iced-x86
    qbopt/blocks.py    which bytes are code, found by reachability
    qbopt/flags.py     which flags are live
    qbopt/lift.py      decoded code as 32-bit values, and back to bytes
    qbopt/calls.py     the runtime calls, and what replaces them
    qbopt/relocate.py  what has to change when code moves
    qbopt/rewrite.py   the driver: an .OBJ in, an .OBJ out
    qbopt/price.py     cycle costs per part, 486 through Core
    fixtures/omf/      real BC output, with a manifest saying what made it
    suite/             the programs the differential runs
    tools/             the DOSBox harness, the corpus generator, the mutations

Start with `docs/testing.md`.

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

    uv run pytest                                  everything, DOSBox included
    uv run pytest -m "not e2e"                     host only, seconds
    uv run python tools/census.py                  what the pass makes of the corpus
    uv run python -m qbopt.rewrite F.OBJ -o G.OBJ  the pass itself

`docs/testing.md` has the tiers and what each needs.
