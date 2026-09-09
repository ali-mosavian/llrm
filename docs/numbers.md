# Numbers

Every row carries what produced it. See `docs/measurement.md` for what each
kind of number does and does not mean.

## Runtime refresh — 2026-09-10, revision b040b61

Subsequent invariant-index hoisting lowers FPBENCH to **5,725.080 ms**;
see [the change and its before/after assembly](invariant-shift.md).

Three baseline runs followed by three optimized runs per benchmark, VBDOS
`v-g3`, tuning CPU `386`, `conf/pinned.conf`, native-FPU replacement off.
Both compiler logs report zero severe errors; both links completed without
errors. Optimization required LIR emission. Every run completed with DONE;
optimized printed answers matched baseline (the same output-precision limits
described below apply). This is two benchmarks, not full-suite timing.

| Program | Steps | Baseline median | Optimized median | BASE/OPT |
|---|---:|---:|---:|---:|
| nbody | 25,000 | 12,360.109 ms | 3,678.857 ms | 3.3598x |
| fpbench | 50,000 | 7,768.941 ms | 5,820.381 ms | 1.3348x |

Raw PIT ticks:

- nbody baseline: `14747860, 14747858, 14747860`; optimized:
  `4387656, 4389546, 4390658`.
- fpbench baseline: `9269760, 9269754, 9269760`; optimized:
  `6945674, 6944774, 6944774`.

nbody is about 0.34% slower than the September 9 optimized median;
fpbench is essentially unchanged. These observations do not identify a
responsible pass and are not real-CPU latency measurements. In particular,
the recent FPDEEP/FPEMU improvements must not be described as measured
FPBENCH acceleration.

| Optimized artifact | SHA256 |
|---|---|
| nbody OBJ | `44f2b1c6c678f91c20e6481d7be93c95784acad51f251d9812dfb776d62df97d` |
| nbody EXE | `a1a2c8a7fca26cd285b93baa5a7e4e8a94160d33a8e87bee467b3dda73786e75` |
| fpbench OBJ | `913abf64465fa5db450a84358b256ef42a3bc06b619960365d375b5c39df84f8` |
| fpbench EXE | `0ec3907e2e980d16290a6afa57a132064972942656dc8ce65f2a548852c62146` |

Build logs, executables and individual outputs are retained locally under
`/var/folders/zp/jrq41dpn4kjcmx0g8lpzx4880000gn/T/qbopt-runtime-current-2wnpyctg`.

## Current nbody runtime check — 2026-09-09

`bench/nbody.bas`, VBDOS `/O /FPi /R /G3 /E /Zi`, 25,000 steps,
five paired repetitions, `conf/pinned.conf`, tuning CPU `386`, native-FPU
replacement disabled. BASE is BC's unchanged object; OPT must report
`wholeseg.Emission.LIR`, not fallback. All 24 final position/velocity values
and DONE matched in every pair. This is benchmark coverage, not the full suite.

| | BC original | Optimized |
|---|---:|---:|
| Median PIT ticks | 14,747,860 | 4,374,864 |
| Median milliseconds | 12,360.109 | 3,666.552 |
| Tick spread (max minus min) | 2 | 2,366 |
| Millisecond spread | 0.002 | 1.983 |

BASE/OPT = **3.3710x** in DOSBox. This does not establish 386/486/P5/P6
hardware speedups: the emulator does not model those instruction latencies.
The earlier optimized timer returning zero was a compiler defect, not a
speedup; `docs/timing-audit.md` records the stage-level diagnosis and fix.

Raw BASE ticks: `14747860, 14747858, 14747860, 14747860, 14747860`.
Raw OPT ticks: `4373286, 4374896, 4373286, 4374864, 4375652`.

Reproduction stamp: revision `6020221` plus the `raising_bytes` fix committed
with this measurement; macOS 27.0 arm64; DOSBox-X 2026.06.02 SDL2.
The compiled BC input is `fixtures/bench/nbody-v-g3.obj`.
SHA256 values:

| Artifact | SHA256 |
|---|---|
| DOSBox-X executable | `dc6d3e9517099b6f4868786fd541de3b099eb3e89f9e49cb4650e4dfe05d12c2` |
| Pinned configuration | `6683b8921c4f410f2eeed9c454ebedce03e3587ca7aa9637471b0190fc602c0f` |
| BC.EXE | `fa8a089bb6ec4a5dcd81705d704929e7c41894efcc09dc453e7e0311f9331efb` |
| LINK.EXE | `2b5236e3f6198a5c4cebec02178786cdcea423f9b6b9823d538a1838e65f045c` |
| VBDCL10E.LIB | `59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301` |
| Input OBJ | `a2b4d2939696f613120db4f01efe05e1e0dd80e0f0dc19948e753041bc9a1d3a` |
| BASE.EXE | `bf57ef07f94e24f9dbbd90d66e84bc05e9c5152b8848f2dc7bf89ba27abfb546` |
| OPT.EXE | `a71a8072f12f04a9ac0cc30bf9b298973985a0443e3bf7cbb47d1aa6aa649397` |

## Current floating benchmark — 2026-09-09

`bench/fpbench.bas`, revision `a24fc54`, VBDOS `/O /FPi /R /G3 /E /Zi`,
50,000 steps, five paired repetitions. Same pinned configuration, host,
emulator and toolchain hashes as the nbody measurement above. Both compile
and link logs were checked; optimization reports LIR emission, not fallback.
Native-FPU replacement is **off** in both versions.

| | BC original | Optimized |
|---|---:|---:|
| Median PIT ticks | 9,269,760 | 6,945,566 |
| Median milliseconds | 7,768.941 | 5,821.045 |
| Spread, milliseconds | 0.003 | 0.964 |

BASE/OPT = **1.3346x in DOSBox**, not a hardware latency measurement.
All twelve printed coordinates and DONE matched in every pair. The source
prints positions rounded to thousandths; this is not a bitwise check of
every floating value or of the velocities.

Raw BASE ticks: `9269760, 9269760, 9269760, 9269758, 9269762`.
Raw OPT ticks: `6944778, 6945566, 6945860, 6945924, 6944774`.

The reachable x87 opcode counts are unchanged: 18 FLD, 14 FSTP, seven FADD,
six FMUL, three FDIV, two FSUB, two FSUBP, one FISTP and one FADDP.
WAITs fall from 15 to 11. Total reachable instructions, including timer
and cold code, grow from 321 to 403. These static facts do not isolate
which transformation accounts for the dynamic gain.

Before and after for the distance calculation (emulator operations shown
as their x87 equivalents; symbolic operands resolved from fixups):

```asm
; both versions retain this arithmetic
fld  dword [deltaX]
fmul dword [deltaX]
fld  dword [deltaY]
fmul dword [deltaY]
faddp
fadd dword [one]
fstp dword [dist2]
; BC follows this with WAIT; optimized code proceeds without that WAIT
```

The arithmetic optimization gap remains; do not describe this as improved
floating CSE or a fully optimized integrator. Full stages are in
`/tmp/qbopt-fpbench-current-stages`.

| Artifact | SHA256 |
|---|---|
| `fixtures/bench/fpbench-v-g3.obj` | `24b93d10ff0b16c00ffa39ceff4e29446021958c7f5fcc28fd7892cc9a0918d1` |
| Optimized OBJ | `b11b99d1ef80493f412472252b70a63d12bc797839728ed94c2cfa2b74bcd3bb` |
| BASE.EXE | `0a3b75b9abd194d7cd7f26a5605f414f947c9dd70c6255d49f2f6e2a182d2431` |
| OPT.EXE | `76d17625a0373da4886c04145982813e813b8a7745dbb09cc651ce618d050bd3` |

## Static: what the pass does to the corpus


Measured 2026-08-30, over the 110 objects in `fixtures/omf`, with
`uv run python tools/census.py`, after `docs/residue.md`'s D, I and B all
landed.

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
`bench/nbody.bas` case documented in `docs/residue.md`'s own D section). I
and B are not visible in this table at all: both need either cross-block
liveness a call sits in the middle of (I) or a widened region's restore
sitting directly against a `consume()`-absorbed call (B), neither of which
the 110 small, single-statement-per-fixture objects in this corpus happen to
produce -- `bench/nbody.bas`'s own before/after numbers, in `docs/residue.md`,
are where their real payoff shows.

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
objects, 2026-08-30): `bench/nbody.bas`'s rewritten object was, at that point,
**80 bytes larger** than BC's own, 1440 against 1360 (+5.9 per cent), not
smaller. Most of that growth was `f2b6f05`'s own necessary correctness fix to
compare absorption (below), which this one file happens to exercise more
heavily, relative to its size, than the corpus average; the remainder was
`docs/residue.md`'s own patterns B, D, E, F and I.

**Re-measured again 2026-08-30**, after D, I and B all landed (same day, same
document): `bench/nbody.bas`'s rewritten object is now **35 bytes larger**
than BC's own, 1395 against 1360 (+2.6 per cent) -- D, I and B accounted for
45 of the 80 bytes (-5, -16, -24, in that order; see `docs/residue.md`'s own
Priority table for the full progression). Still not a net win on this one
file: what remains is F and E, both still architectural and open, plus I's
own 4 still-open instances (a second, post-rewrite liveness pass, out of this
round's scope). The corpus-wide 19 per cent smaller above is a real aggregate
and does not average out per-file --
`bench/nbody.bas` is the one program tracked closely enough in this document
to know it currently regresses.

**Re-measured a third time 2026-08-30**, after E's own closure (`docs/residue.md`'s
own E section has the mechanism and the worked example): `bench/nbody.bas`'s
rewritten object is now **26 bytes larger** than BC's own, 1386 against 1360
(+1.9 per cent) -- E alone, -9 bytes. F's own recognition gap closed too
(register- and memory-sourced `cwd`, `Op.MOVSX`), but it does not move this
object's byte count at all: every one of its 9 measured sites is still
refused by the same growth check that refuses any region wider than what it
replaces, for reasons `docs/residue.md`'s own F section now measures in
full rather than estimates. The 110-object `fixtures/omf` static census
(below) is unchanged by either -- neither shape occurs in that corpus.

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
self-contained call. See `qbopt/frontend/stack.py`'s `frames()` and `calls.py`'s
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
corpus cases. See `qbopt/objectfile/relocate.py`'s `crossed_pair` and
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
boundary fix; the region count in this paragraph is the current static census.)

### Re-run 2026-09-01, and it had stopped working

Re-measured from BC's own objects (`build/vbd-qbopt-base`), all 15 modules
through the current pass, linked with the recovered response file and run
headless against `dm3ish.bsp`.

| | BC only | qbopt |
|---|---|---|
| qrender.exe | 285,678 | 285,774 |
| frames | 271 | **291** |
| peak fps | 88 | **95** |
| cp_pts | 216 | 216 |

**Before the fix in this session it did not run at all.** It linked cleanly
and then corrupted itself -- `String space corrupt`, or a hang. One module
of the fifteen, `sys.obj`, and one transform: widening dropped the segment
override off a far pointer, so

    mov ax,es:[bx] ; mov dx,es:[bx+2]     became    mov eax,[bx]

and the program read and wrote `ds` where BC wrote `es`. `lift.memory()`
named Space.SEGMENT, FRAME and GROUP and let Space.FAR fall into a bare
operand. `select.py` had handled the same address correctly all along.

Nothing caught it because nothing was looking: `tools/mutate.py` has carried
a `segment-override-not-refused` mutation since before an override was
resolved rather than refused, and its pattern stopped matching when that
line was rewritten. It reported "pattern appears 0 times" -- which counts as
a caught mutation and checks nothing. Repointing it made the gate 40 of 40
for the first time.

The same figures at `cycles=max` are worth less than they look: both builds
race the host, so the frame counts are a comparison of two unpinned runs.
Fixed cycles is what makes them a measurement.

This is the correctness result, not a speed result. The speed is behind the
refusals above.

### What emitting the segment costs

Re-measured 2026-08-31, code bytes only, `v-g3`.

| | fixtures/omf (155) | qb-qrender (15) |
|---|---|---|
| BC | 95,189 | 74,855 |
| absorbed | 89,665 (-5.80%) | 76,198 (+1.79%) |
| absorbed, then MIR-written | 89,464 (-6.01%) | 75,582 (+0.97%) |

**Writing the segment from MIR is not a size cost. It pays for about half of
what absorption costs qb-qrender**, and it is what `rewrite.py` does by
default. Getting the size there took four encodings:
the byte-immediate push, a byte displacement through a base register, the
accumulator's own arithmetic opcode, the by-1 shift, and the byte immediate
in a memory compare.

Before those, the same table read +2,776 bytes for qb-qrender, +3.6% on top
of absorption. **The corpus could not see any of it** -- it came out 3 bytes
shorter either way, because suite programs push addresses rather than small
constants and index nothing through a register. Every one of those five
encodings is something BC writes constantly in a real program and almost
never in a thirty-line one.

## Modelled

`qbopt/price.py` prices what is in the object. That is the right answer for a
widened region and the wrong one for an absorbed call: BC's side is three
instructions and the routine behind the call is in the runtime library, so an
absorbed call reads there as a large loss and is not one.

For the calls, `python -m qbopt.cycles.cycles` carries the routine bodies. All
four rows below are real: the `stock` rows are BC's own call plus the actual
runtime routine, confirmed 2026-08-30 byte-for-byte against VBDOS's
`VBDCL10E.LIB` (`..\rt\helpi4.asm`, offset 0x1d00); the `qbopt absorbed` rows
are `Emitted.code` taken directly from `qbopt/legacy/calls.py`'s `absorb()` /
`dividing()` -- what this pass emits today, not a hand-written guess. Standing
alone, in cycles:

| | ins | 486 | P5 | P6 | K5 | K6 | K7 | Core |
|---|---|---|---|---|---|---|---|---|
| multiply, `call B$MUI4` fast path | 14 | 68 | 34 | 42 | 11 | 12 | 14 | 43 |
| multiply, `call B$MUI4` full path | 22 | 103 | 62 | 45 | 15 | 15 | 19 | 45 |
| multiply, qbopt absorbed | 5 | 40 | 18 | 14 | 11 | 10 | 13 | 14 |
| divide, `call B$DVI4` (÷256 short path) | 33 | 138 | 91 | 94 | 63 | 66 | 68 | 91 |
| divide, qbopt absorbed | 7 | 62 | 58 | 48 | 44 | 43 | 43 | 33 |
| compare, `call B$CPI4` | 20 | 72 | 34 | 47 | 16 | 16 | 18 | 48 |
| compare, qbopt absorbed | 4 | 12 | 9 | 11 | 8 | 8 | 9 | 12 |
| remainder, `call B$RMI4` (MOD 256 short path) | 31 | 133 | 89 | 93 | 63 | 65 | 67 | 91 |
| remainder, qbopt absorbed | 8 | 64 | 60 | 48 | 44 | 43 | 43 | 33 |

Every `qbopt absorbed` row above is the memory-resident operand shape (`mov
eax,[a]` against a static address); a register-resident operand (both sides
already popped off the stack by `consume()`, e.g. `B$MUI4` behind an
expression rather than a bare variable) costs one instruction more standing
alone but is otherwise the same shape -- `cycles.py`'s own `CASES` dict
carries both. `B$MUI4`/`B$DVI4`/`B$RMI4` in the real library are each a
5-byte far jmp thunk into `__aFlmul`/`__aFldiv`/`__aFlrem`; `B$CPI4` alone has
its own inline body. `B$RMI4` had no case here before this table -- MOD was
simply missing.

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

**What this table does not price: the routine's own bytes staying linked in
regardless.** `bench/nbody.bas` drops all 21 calls into
`B$MUI4`/`B$DVI4`/`B$CPI4` -- every reference to that 262-byte runtime
module (`runtime/rt/helpi4.asm`, shared with the never-called `B$RMI4`) is
gone from `NBODYQ.OBJ`. Measured directly against the linked `.EXE`s: the
module's own code is byte-identical and present in *both* `BASE.EXE` and
`OPT.EXE` (`build/bench/v-g3`) -- something else in the runtime pulls it in
either way, not this program's own calls. The cycle price above is real cost
this program no longer pays at runtime; it is not bytes the linked program
stops carrying, and `BASE.EXE`/`OPT.EXE`'s own sizes (34232 / 34280) show it:
`OPT.EXE` is 48 bytes larger, not smaller, once the routine bodies BC still
links are counted alongside `NBODY.OBJ`'s own code.

## Dynamic

`bench/nbody.bas`, `v-g3`, 25000 steps, 7 repetitions, `conf/pinned.conf`,
re-measured 2026-08-31 after divide strength reduction landed. Read via the
8253, not `TIMER` -- see `docs/measurement.md` for why this reads as a
spread rather than an exact repeat.

| | base | opt |
|---|---|---|
| ticks (median) | 14747860 | 4931572 |
| ms | 12360.1 | 4133.1 |
| spread | 2 (0.00001%) | 2 (0.00004%) |

**Base is 2.99 times slower than opt** (`base/opt` = 2.9905) -- and this is
the one figure in this document that went *down* on purpose. It was 3.26
the day before. The cause is divide strength reduction, and the reason to
keep it anyway is the subject of the next section.

### Floats

`bench/fpbench.bas` is the same integrator in SINGLE, `v-g3`, 4000 steps, 5
repetitions, measured 2026-08-31.

| | base | opt | opt `--native-fpu` |
|---|---|---|---|
| ticks (median) | 699296 | 699304 | 371660 |
| ms | 586.1 | 586.1 | 311.5 |
| ratio to base | -- | 1.0000 | 1.8815 |

**The pass does nothing for float code.** Not a small win rounded away: 8
ticks out of 699296, well inside the spread. That is what it should be --
everything measured above absorbs calls into `B$MUI4`, `B$DVI4` and
`B$CPI4`, and float code makes none of them. It goes through the x87
emulator instead, which is untouched unless `--native-fpu` is on.

With `--native-fpu`, **base is 1.88 times slower than opt**, and every
printed coordinate matches BC's own build. Which is the whole benefit
available on floats today, from replacing the emulator's `int 34h`..`3Dh`
with the x87 instruction that was always meant to be there.

Both numbers are on code that computes the right answer. An earlier
measurement of 1.78 was not: `forward.py` was deleting the second of two
`fld dword ptr [si]`, and the build it timed printed -2147483648 for every
coordinate. It ran faster because it was doing less, and less was wrong.
`suite/fpdeep.bas` exists so that shape is in the corpus now.

## Absorption

All 21 of `bench/nbody.bas`'s arithmetic call sites -- 11 `B$MUI4`, 6
`B$DVI4`, 4 `B$CPI4` -- are absorbed; the rewritten object contains none of
them. Base is unchanged, as it must be
-- BC's own build does not move. `bench/nbody.bas` avoids `fixMul&` by
construction (Q23.9, see `suite/nbody.bas`'s own comment) specifically so BC
alone can build the base half of this comparison.

Re-measured, not reasoned: opt dropped from 4649876 to 4519562 ticks (2.8%
faster) once E and F actually landed, superseding the earlier note that
predicted no change from E alone (E widens an interleaved statement rather
than removing a call from the hot path, but the widened form itself still
runs fewer cycles per iteration -- one fewer memory round trip through the
16-bit halves, which the reasoning at the time did not account for).

This is the dynamic side of a fact the static census already shows: the
object is still 26 bytes larger than BC's own build (`build/bench/v-g3`,
1386 against 1360 after E's own closure -- see the residue.md-driven fixes
above), yet opt itself runs 33% faster than it did at `9c02db9` (6750212 ->
4519562 ticks). Byte count and cycle count are different axes -- G+H, D, I,
B and E all remove calls, restores, round trips or extra memory traffic from
the *hot path*, which is what the timer reads, not what shrinks the object.

### Where DOSBox and the cycle model disagree, and which to believe

The 3.26 -> 2.99 drop is entirely divide strength reduction, established by
running the benchmark with `_power_of_two()` forced to None and nothing else
changed:

| | opt ticks | ratio |
|---|---|---|
| `idiv` kept | 4518806 | 3.2637 |
| shift sequence | 4931572 | 2.9905 |

4518806 against the 4519562 measured the day before, so the attribution is
not an inference.

DOSBox charges one price per instruction and models no latency, so trading
one `idiv` for a four-instruction shift sequence reads as 9 per cent slower
there. `qbopt/cycles` holds published latencies, and prices the same two
objects -- same absorbed call in both, the divide form the only difference:

| | 486 | P5 | P6 | K5 | K6 | K7 | Core |
|---|---|---|---|---|---|---|---|
| `idiv` kept | 1068 | 717 | 360 | 297.4 | 306 | 300 | 234.4 |
| shift sequence | 889 | 513 | 129.1 | 91.9 | 107.1 | 106.1 | 87.9 |
| speedup | 1.20x | 1.40x | 2.79x | 3.24x | 2.86x | 2.83x | 2.67x |

Faster on every machine modelled, by 1.2x on the 486 this code was written
for and by nearly 3x on anything later. A 32-bit `idiv` is 43 cycles on a
486 and DOSBox charges it the same as an `add`.

So the 2.99 figure is kept and quoted as what DOSBox measures, not as what
the code costs. Where the two disagree the cycle model is the one about
hardware, and this document quotes both rather than picking the flattering
one.

One honest gap: `bench/nbody.bas` itself has no golden and prints only
`TICKS=`, so nothing here checks its own arithmetic. `suite/nbody.bas` --
the same integrator, Q16.16 instead of Q23.9 -- is golden-checked across all
twelve configurations by `tools/matrix.py`/`tests/test_e2e.py`, and passes;
that is the evidence this number rests on for correctness, not an
independent check of `bench/nbody.bas`'s own object.

This number moved three times before landing here, and every move is worth
recording rather than only the final figure. The first measurement, before
`2 ^ DAMP` was replaced with the literal `16` it always evaluated to, read
8.25 per cent -- diluted by two calls per body per step into `B$POW4`, a
floating-point routine absorption was never going to touch. The second,
before `calls.py` could recognise a register-resident or stack-stranded
operand at all (see `qbopt/frontend/stack.py`), read 13 per cent. The third, 2.18x at
`9c02db9`, was real but measured before this session's comparison-absorption
correctness fix and the phase-2 IR work that followed it. None was a wrong
measurement; each was measuring a program, or a pass, that was not yet what
it should have been.

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
