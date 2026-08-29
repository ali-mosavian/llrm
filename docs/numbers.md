# Numbers

Every row carries what produced it. See `docs/measurement.md` for what each
kind of number does and does not mean.

## Static: what the pass does to the corpus

Measured 2026-08-29, over the 110 objects in `fixtures/omf`, with
`uv run python tools/census.py`.

| | |
|---|---|
| objects | 110, 71204 bytes of code |
| mapped | 110; none refused |
| regions found | 1332 |
| taken | 1237 |
| their bytes | 24009 -> 15321, 36 per cent smaller |

Refused, by reason:

| count | why |
|---|---|
| 73 | the region crosses a LEDATA boundary |
| 16 | a single pair that widens to more bytes than BC wrote |
| 6 | a line number points inside the region |

Every module maps. It did not before: the entry point was searched for, and the
search picked one byte late on 22 objects and could not explain the `/V /W`
event stub at all. Both are settled by the module header the QuickBASIC 4.5
runtime defines -- see `docs/testing.md` and `AGENTS.md`.

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

### Built, linked and run

All 15 modules rewritten, linked against the patched uGL and run under the
pinned profile, `-campath -ticks 200` against `dm3ish.bsp`. `-ticks` is what
makes the comparison mean anything: the simulation steps at a fixed `HOST_DT`
and stops on the budget, where `-bench` counts frames and lets a faster build
walk further before it stops.

| | base | opt |
|---|---|---|
| qrender.exe | 285678 | 285550 |
| BENCH.BMP | — | byte for byte identical to base |
| polys, tris | 240, 600 | 240, 600 |
| px, py, pz | -119.6837, -438.5061, 184.7778 | identical |
| ticks, cp_pts, clp_cnt | 202, 216, 1648 | identical |

**No measurable speed difference.** frames, seconds and every `ft_*` and `fps_*`
field came out bit-identical across the two runs -- the timer quantises, and 63
regions over 924 bytes of a 74,873-byte program is too small a fraction of the
work to show through it. A `-bench 60` run reported 59.14 ms against 58.12,
1.7 per cent, but the two runs were not at the same place in the map by then and
drew different geometry, so that pair is not a measurement of anything.

This is the correctness result, not a speed result. The speed is behind the
refusals above.

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

That is what absorbing a call buys, and it is why divide is taken even where
it grows: eighteen bytes for the divide and twenty-one for the remainder,
against a call site of twenty-one, or fifteen under `/G3`. Divide's routine is worse than
multiply's: it normalises its operands one bit at a time, twelve instructions
per pass and up to fifteen passes.

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
