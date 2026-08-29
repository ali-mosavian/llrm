# Numbers

Every row carries what produced it. See `docs/measurement.md` for what each
kind of number does and does not mean.

## Static: what the pass does to the corpus

Measured 2026-08-29, over the 90 objects in `fixtures/omf`.

| | |
|---|---|
| regions found | 1131 |
| taken | 876 |
| their bytes | 17071 -> 12063, 29 per cent smaller |

Refused, by reason:

| count | why |
|---|---|
| 108 | `B$DVI4` is not absorbed |
| 69 | `B$RMI4` is not absorbed |
| 60 | the region crosses a LEDATA boundary |
| 10 | widening it grows 6 bytes to 8 |
| 4 | a line number points inside the region |

Divide and remainder are refused on measured grounds rather than for want of
work: see the trap figures in AGENTS.md.

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

## Modelled: cmpord-v-g3, 48 absorbed comparisons

`uv run python -m qbopt.price fixtures/omf/cmpord-v-g3.obj`, qbopt at 06e7967.
Published latencies, so a ranking.

| | 486 | P5 | P6 | K5 | K6 | K7 | Core |
|---|---|---|---|---|---|---|---|
| back to back | 5.60x | 2.00x | 31.00x | 9.00x | 6.71x | 8.14x | 45.00x |

Instructions 144 -> 96, 1.50x. The very large figures are what removing a far
call and the routine behind it looks like to a model that prices the call; they
are not a claim about wall-clock.

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
