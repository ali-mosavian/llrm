# Numbers

Every row carries what produced it. See `docs/measurement.md` for what each
kind of number does and does not mean.

## Static: what the pass does to the corpus

Measured 2026-08-30, over the 110 objects in `fixtures/omf`, with
`uv run python tools/census.py`, after `docs/residue.md`'s D landed.

| | |
|---|---|
| objects | 110, 71204 bytes of code |
| mapped | 110; none refused |
| regions found | 1404 |
| taken | 1386 |
| their bytes | 26446 -> 21302, 19 per cent smaller |

Refused, by reason:

| count | why |
|---|---|
| 12 | a single pair that widens 6 bytes to 8 |
| 6 | a line number points inside the region |

16 bytes bigger before, 16 bytes bigger after, than the census immediately
before this (26366 -> 21286) -- taken went 1382 -> 1386 and the "widens 7
bytes to 8" refusal category (4 instances) disappeared outright: D's own
immediate-operand ALU pairs, previously invisible to `classify()`, are what
those 4 single-pair regions actually were -- recognising them either lets the
pair widen on its own where it used to lose to its own restore, or reunites
it with a neighbouring load/store into one bigger region entirely (the
`bench/nbody.bas` case documented in `docs/residue.md`'s own D section).

133 bytes bigger, 76 bytes bigger, than the census immediately before this
(26233 -> 21210) -- neither is a regression. Region *count* is unchanged
(1404 found, 1382 taken): commit 3 (docs/residue.md's G and H) never adds or
removes a region, it widens 19 existing call-absorption regions, corpus-wide,
into a call *plus* the widened tail BC's own code reads directly out of
eax:edx right after it, the same way it would after a real call. Those tail
bytes were never part of any region's own before/after accounting until now
-- widening across the call's restore is what commit 1 (`ir.py`) made
representable at all, per `RESTORE_EFFECTS`'s own documented fact that the
restore leaves the 32-bit result untouched. Net, corpus-wide: 57 more bytes
saved than before (5080 against 5023). `bench/nbody.bas`'s own VBDOS `/G3`
object, not part of this census, shows the real payoff more clearly: 12
combined call+tail edits, 59 bytes saved over what standalone call absorption
alone would have produced.

That 59-byte win is against what standalone absorption alone would have cost
on this file, not against BC's own code -- and it is not enough to make this
one program a net win yet. Measured directly (`module.of(...).code` on both
objects, 2026-08-30): `bench/nbody.bas`'s rewritten object is currently **80
bytes larger** than BC's own, 1440 against 1360 (+5.9 per cent), not smaller.
Most of that growth is `f2b6f05`'s own necessary correctness fix to compare
absorption (below), which this one file happens to exercise more heavily,
relative to its size, than the corpus average; the remainder is
`docs/residue.md`'s own still-open patterns B, D, E, F and I, re-measured
fresh in that document as of the same date. The corpus-wide 19 per cent
smaller above is a real aggregate and does not average out per-file --
`bench/nbody.bas` is the one program tracked closely enough in this document
to know it currently regresses.

4268 bytes bigger than the previous census (16942), from a correctness fix
to compare absorption in `calls.py`, not a lost optimisation. `B$CPI4`'s own
"Uses: ax,cx,dx,bx" comment overstates what a real call actually clobbers --
its body (`runtime/rt/helpi4.asm`) never touches cx, dx or bx, and its
cProc save-list preserves ax too, so a real call changes nothing but the
flags. BC's own code can keep a value live in eax right across a compare
embedded in a larger expression, which absorbing straight into eax silently
destroyed -- found by `tools/fuzzcheck.py`'s generated corpus (F014, F017).
The fix wraps the scratch register absorption still needs in push/pop, four
bytes a plain load-and-cmp did not carry before; a compare popped off the
stack rather than reloaded needs a heavier bp-relative save/restore for the
same reason, described in `calls.py`'s `compare_consume()`. Every region
taken and every byte this pass was already correct about is unchanged --
only the compare sites' own cost moved.

472 bytes smaller than the census before that (17414), from two of
`docs/residue.md`'s patterns, both plain codegen bugs rather than new
capability: `popped_into()` was recombining two words already contiguous on
the stack through five wasted instructions (pattern A, corpus-wide, not just
the one object residue.md measured it on), and `absorb()` reloaded `x*x`'s
address twice instead of once (pattern C). Neither changed what any region
computes.

67 more regions than the census before that, all of them call sites
`calls.py` could not previously classify at all: an operand pushed from a
register rather than reloaded from memory (with or without a backing
store), or one stranded on the stack under an entirely separate,
self-contained call. See `qbopt/stack.py`'s `frames()` and `calls.py`'s
`consume()`.

Every module maps. It did not before: the entry point was searched for, and the
search picked one byte late on 22 objects and could not explain the `/V /W`
event stub at all. Both are settled by the module header the QuickBASIC 4.5
runtime defines -- see `docs/testing.md` and `AGENTS.md`.

No region is refused for crossing a LEDATA boundary any more -- it was the
largest refusal category, 73 of 1337. `relocate()` moves the shared boundary
to the edit's own edge instead of merging the two records: neither is removed,
so no FIXUPP is re-parented to whatever LEDATA happens to precede it after the
edit, which is the failure mode an earlier, merging design hit on 72 of 73
corpus cases. See `qbopt/relocate.py`'s `crossed_pair` and
`_boundary_overrides`, and AGENTS.md's "Moving code across a LEDATA boundary".

## The real program

qb-qrender, 15 modules, 10,734 lines of BASIC, 74,873 bytes of BC output.
Compiled under `v-g3`; census with `tools/census.py`.

| | |
|---|---|
| modules mapped | 15 of 15 |
| regions found | 164 |
| taken | 65 |
| their bytes | 949 -> 824 |

Two findings, and the second is worth more than the first.

**The program declares no `LONG` at all**, so the pass finds 164 regions in
74,873 bytes where the suite finds 1131 in 69,097. That is the project's own
thesis rather than a disappointment: longs are avoided in the code that would
most benefit from them, and the avoidance is what the pass exists to remove.

**Before the decoder understood the FP emulator, reachability explained 2 of
the 15 modules.** `int 34h`..`3Bh` is an x87 instruction with its operand
inline, and there are 2130 of them here. Nothing in the suite has one. This is
what measuring against a real program was for.

Of what is left refused, 98 of 99 are single pairs that widen to more bytes
than BC wrote -- the shape a whole-procedure rewrite would fix and a per-region
one cannot. None cross a LEDATA boundary any more.

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
field came out bit-identical across the two runs -- the timer quantises, and 65
regions over 949 bytes of a 74,873-byte program is too small a fraction of the
work to show through it. A `-bench 60` run reported 59.14 ms against 58.12,
1.7 per cent, but the two runs were not at the same place in the map by then and
drew different geometry, so that pair is not a measurement of anything.

(The table above is from the specific build the run used, before the LEDATA
boundary fix; the region count in this paragraph is the current static census.
Re-running qb-qrender would take slightly fewer bytes now, not a different
conclusion.)

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
instructions 144 -> 192, 0.75x -- more instructions than BC emitted, not
fewer, and worse than the 144 -> 96 an earlier census measured here. That
earlier count was the bug this session fixed: a bare load-and-cmp is what a
comparison absorbs to only if eax is free to clobber, and it was not (see
the Static section above). The push/pop wrap correctness needs turns three
instructions (BC's own `push / push / call`) into four; the call itself
still leaves, so `price.py`'s own back-to-back model -- which is what
overlaps with a program's other work, unlike the standing-alone one -- still
shows a real speedup in cycles even where the plain instruction count does
not. The cycle columns from `price.py` on that object are not reproduced
here, for the reason above.

## Dynamic

`bench/nbody.bas`, `v-g3`, 25000 steps, 7 repetitions, `conf/pinned.conf`,
measured 2026-08-29 at qbopt `9c02db9`. Read via the 8253, not `TIMER` -- see
`docs/measurement.md` for why this reads as a spread rather than an exact
repeat.

| | base | opt |
|---|---|---|
| ticks (median) | 14747858 | 6750212 |
| ms | 12360.1 | 5657.3 |
| spread | 2 (0.00002%) | 12168 (0.18%) |

**Base is 2.18 times slower than opt** (`base/opt` = 2.1848) -- the first
genuine execution-timing number this project has had, closing the "No dynamic
number exists" item. `bench/nbody.bas` avoids `fixMul&` by construction
(Q23.9, see `suite/nbody.bas`'s own comment) specifically so BC alone can
build the base half of this comparison.

This number moved twice before landing here, and both moves are worth
recording rather than only the final figure. The first measurement, before
`2 ^ DAMP` was replaced with the literal `16` it always evaluated to, read
8.25 per cent -- diluted by two calls per body per step into `B$POW4`, a
floating-point routine absorption was never going to touch. The second,
before `calls.py` could recognise a register-resident or stack-stranded
operand at all (see `qbopt/stack.py`), read 13 per cent -- most of the hot
loop's calls were simply not being absorbed. Neither was a wrong
measurement; both were measuring a program, or a pass, that was not yet
what it should have been.

dosbox-x 2026.06.02 SDL2; `conf/pinned.conf` sha256
`6683b8921c4f410f2eeed9c454ebedce03e3587ca7aa9637471b0190fc602c0f`; VBDOS
BC.EXE sha256 `fa8a089bb6ec4a5dcd81705d704929e7c41894efcc09dc453e7e0311f9331efb`.

## Predecessor, runtime pass, reconstructed configuration

From the inherited documents. The runtime pass rewrote the loaded image; these
did not come from qbopt and are not comparable to a row above without care.

| long / integer | BC | after the runtime pass | goal |
|---|---|---|---|
| bitwise, additive | 1.96 | 1.37 | 1.0 |
| multiply, divide | 5.62 | 3.27 | 1.0 |
