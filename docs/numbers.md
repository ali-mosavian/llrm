# Numbers

Every row carries what produced it. See `docs/measurement.md` for what each
kind of number does and does not mean.

## Static: what the pass does to the corpus

Measured 2026-08-29, over the 110 objects in `fixtures/omf`, with
`uv run python tools/census.py`.

| | |
|---|---|
| objects | 110, 71204 bytes of code |
| mapped | 102; 8 refused |
| regions found | 1166 |
| taken | 1080 |
| their bytes | 21131 -> 13530, 35 per cent smaller |

Refused, by reason:

| count | why |
|---|---|
| 66 | the region crosses a LEDATA boundary |
| 14 | a single pair that widens to more bytes than BC wrote |
| 6 | a line number points inside the region |

The 8 unmapped are `/V /W` builds whose event stub sits in the header at an
offset no record names.

## The real program

qb-qrender, 15 modules, 10,734 lines of BASIC, 74,873 bytes of BC output.
Compiled under `v-g3`; census with `tools/census.py`.

| | |
|---|---|
| modules mapped | 15 of 15 |
| regions found | 164 |
| taken | 63 |
| their bytes | 924 -> 800 |

Two findings, and the second is worth more than the first.

**The program declares no `LONG` at all**, so the pass finds 164 regions in
74,873 bytes where the suite finds 1131 in 69,097. That is the project's own
thesis rather than a disappointment: longs are avoided in the code that would
most benefit from them, and the avoidance is what the pass exists to remove.

**Before the decoder understood the FP emulator, reachability explained 2 of
the 15 modules.** `int 34h`..`3Bh` is an x87 instruction with its operand
inline, and there are 2130 of them here. Nothing in the suite has one. This is
what measuring against a real program was for.

Of what is left refused, 96 of 101 are single pairs that widen to more bytes
than BC wrote -- the shape a whole-procedure rewrite would fix and a per-region
one cannot.

## Modelled

`qbopt/price.py` prices what is in the object. That is the right answer for a
widened region and the wrong one for an absorbed call: BC's side is three
instructions and the routine behind the call is in the runtime library, so an
absorbed call reads there as a large loss and is not one.

For the calls, `python -m qbopt.cycles.cycles` carries the routine bodies. A
long multiply, standing alone, in cycles:

| | ins | 486 | P5 | P6 | K5 | K6 | K7 | Core |
|---|---|---|---|---|---|---|---|---|
| `call B$MUI4`, fast path | 14 | 68 | 34 | 42 | 11 | 12 | 14 | 43 |
| `call B$MUI4`, full path | 22 | 103 | 62 | 45 | 15 | 15 | 19 | 45 |
| one 32-bit `imul` | 2 | 30 | 14 | 10 | 8 | 7 | 10 | 10 |

That is what absorbing a call buys, and it is why a guarded divide is worth
thirty-six bytes against fifteen. Divide's routine is worse than multiply's: it
normalises its operands one bit at a time, twelve instructions per pass and up
to fifteen passes.

For a widened region, `cmpord-v-g3` with its 48 comparisons absorbed reports
instructions 144 -> 96. The cycle columns from `price.py` on that object are not
reproduced here, for the reason above.

## Dynamic

None yet. `tools/bench.py` and the high-resolution timer are not written, so
there is no row here rather than an unpinned one.

## Predecessor, runtime pass, reconstructed configuration

From the inherited documents. The runtime pass rewrote the loaded image; these
did not come from qbopt and are not comparable to a row above without care.

| long / integer | BC | after the runtime pass | goal |
|---|---|---|---|
| bitwise, additive | 1.96 | 1.37 | 1.0 |
| multiply, divide | 5.62 | 3.27 | 1.0 |
