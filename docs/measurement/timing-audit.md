# Arithmetic timing audit — incomplete

## Current runtime benchmark check, 2026-09-09

A fresh VBDOS `/G3` build of `bench/nbody.bas` has zero severe compiler
errors, but cannot yet provide a current optimized timing. Requiring
`wholeseg.Emission.LIR` (rather than timing rewrite's unchanged fallback)
first refused at `0x0444`: the allocator inserted `mov al,cl` before the
PIT writer's OUT, and the register-move selector supported only words and
dwords. Byte-register moves are now supported with an emitted-code
fail-first regression. The next refusal is `0x045d: mov has 1 fixups and 0
fields to put them in`, also in PITSNAP. No runtime speedup is claimed.

All MIR and machine stages for this investigation were dumped to
`/tmp/qbopt-nbody-timing-stages`. The benchmark source now prints its final
positions and velocities, so the older statement in `numbers.md` that it
prints only ticks no longer describes the current source. Future timings
must compare those answers and require genuine optimized emission.

The `0x045d` refusal was traced to the object reader discarding external
OFFSET16 identities: `B$SEG` became literal address zero. External cells
now retain their EXTDEF identity and emit a relocatable displacement;
different external names remain potentially aliasing. The fresh object
now emits through LIR and both versions link successfully. A 100-step
correctness smoke run completed for BASE, but OPT did not complete within
30 seconds and its redirected output was empty. This is an unresolved
execution failure, not a timing result. The unmodified BC object is kept
in `tests/fixtures/bench/nbody-v-g3.obj` for reproduction.

The 30-second failure used the FAST dynamic-core profile, not the pinned
measurement profile. With the pinned normal core, both programs finished,
but optimized NBODY printed `TICKS=0` even after 25,000 steps. Its coordinates
and velocities matched BASE. The stage dumps localized a high-byte-clear
defect: `mov bl,es:[bx]; xor bh,bh` became a load into AL followed by the
unchanged physical BH clear and a read of an unwritten spill slot. The XOR's
whole-word SSA definition had only opaque operands, not an allocatable result.

The frontend now raises an unobserved high-byte clear as a word-sized mask
with an explicit value result and upper-word preservation. This is allowed
only when its flags are unobserved and overwritten within the block. It
introduces no machine knowledge into optimization passes. Dumps before and
after are in `/tmp/qbopt-nbody-external-stages` and
`/tmp/qbopt-nbody-byte-stages`; the repaired allocated sequence is
`mov al,es:[bx]; and ax,255`. The pinned 100-step run now reports nonzero
ticks and unchanged simulation results. The dynamic-core discrepancy has
not been separately rechecked and is not used as timing evidence.

The CPU selector introduced in 9887b1c was not supported by a sufficiently
precise timing model. Reciprocal division was held uncommitted during the
initial audit. Correct runtime answers do not validate speed predictions.

## Verified findings

Intel's 80386 Programmer's Reference Manual, IMUL entry:

https://www.ardent-tool.com/CPU/docs/Intel/386/manuals/prref386/IMUL.htm

Register-operand word IMUL spans 9–22 clocks; dword spans 9–38. The
sign-extended immediate-byte forms span 9–14. For a positive known
multiplier, the documented early-out formula is
`max(ceil(log2(multiplier)), 3) + 6`. Thus the immediate factor 20 costs
11 core clocks, and 85 costs 13, not the selector's flat 22. This formula
alone does not establish whole-sequence execution cost or prefix effects.
The text rendering loses which register operand is underlined; inspect
the original page before applying it to register-register multiplication.

Intel's IDIV entry distinguishes word (27 clocks) and dword (43):

https://www.ardent-tool.com/CPU/docs/Intel/386/manuals/prref386/IDIV.htm

Intel Architecture Optimization Reference Manual 245127-001, Table 1-1,
printed page 1-8 (PDF page 32), describes Pentium II/III integer multiply
as four-cycle latency and one-per-cycle throughput. These are distinct
quantities; neither justifies assigning every operand form a scalar cost
of four. Printed page 2-16 explains that prefixes increase instruction
length and may constrain decoding. Do not extrapolate this document's
Pentium II/III details to every P6 model without checking.

https://download.intel.com/design/PentiumII/manuals/24512701.pdf

## Required corrections before relying on selection

- Separate instruction form, operand width and known multiplier value.
- Separate latency from reciprocal throughput and resource occupancy.
- Check the actual 16-bit-mode encodings, including operand/address prefixes.
- Compare the selected LEA sequence, not only its earlier shift/add expansion.
- Account for required copies and allocation effects without double-counting.
- Verify 486 and Pentium instruction tables, and exact P6 forms, from primary
  manuals. Existing recollected representative numbers are not that evidence.

The initial HOTLPX P6 choice was corrected to rank the fused LEA sequence;
see arithmetic-targets.md. Whole-sequence clock claims remain provisional.

## First correction

The selector now uses the documented 386 early-out formula for positive
imm8 multipliers (0–127), whose signed value is identical for word and
dword forms. Other forms still use the old estimate pending their audit.
The fail-first regression is factor 85: previously selected abstract chain
`shl 2; add source; shl 2; add source; shl 2; add source`, now direct IMUL.
The chain's current estimate is 17 clocks including a seed move; the
immediate IMUL's documented core cost is 13, not 22.
This is a correction of the multiply input to selection, not a claim that
the whole chain model or final machine schedule has been validated.

The next fail-first check preserves factor ten's existing LEA + SHL choice:
costing the pre-peephole shift/add/shift instead would wrongly replace it
after the immediate correction. The exact three-operation shape recognized
by `peephole.addresses` is ranked as LEA plus SHL on 386. Intel's LEA entry
documents two core clocks:
https://www.ardent-tool.com/CPU/docs/Intel/386/manuals/prref386/LEA.htm
This remains a pre-allocation estimate; prefix and failed-coalescing costs
remain part of the unfinished audit.

## 486 and Pentium primary-table inspection

Intel 240440-002, November 1989 i486 data sheet, Table 10.1
(PDF page 135), gives IMUL ranges of 13–26 for word and 13–42 for
dword operands. Printed page 143, note 3, makes multiplier dependence
explicit and distinguishes positive and negative multipliers. Its logarithm
notation does not explicitly state rounding, so do not invent an exact
non-power-of-two cost from the OCR text. The original footnote was rendered
and inspected. A flat 26 is not an established cost for the large negative
magic multiplier used by division by seven. The pending 486 reciprocal
win (41 versus 48) must not be accepted on that basis.

https://bitsavers.trailing-edge.com/components/intel/80486/240440-002_i486_Microprocessor_Nov89.pdf

Intel 241430-004, July 1995 Pentium Family Developer's Manual Volume 3,
Table F-2, printed F-13 (PDF page 1006), was also rendered and inspected:
immediate and two-operand IMUL are 10 clocks; accumulator-form word
IMUL is 11, dword is 10. IMUL is non-pairable. F-12 lists IDIV as
30 for word and 46 for dword, also non-pairable. These support the old
P5 scalar numbers for the relevant dword register forms, but do not
validate a whole dependency-chain or prefix model. F-6 specifies cache
hits, aligned accesses, available bus, TLB hits and no exceptions among
its assumptions.

https://www.ardent-tool.com/CPU/docs/Intel/Pentium/241430-004_scan.pdf

Next model change: represent instruction-form-specific cost evidence and
ranges, separate from the old scoreboard estimates. A candidate must not
win merely because an unknown multiplier was assigned an arbitrary midpoint.
No emitted code changed during this primary-table inspection.

## Bounds integrated into division selection

`crates/llrm-core/src/backend/timing.rs` now separates audited multiplication ranges and division
widths from the old scoreboard's midpoint guesses. Reciprocal selection
uses the maximum multiply cost and minimum divide cost. Missing exact-form
evidence (currently P6 and later profiles) retains IDIV. This is not a
claim that reciprocal division cannot win on those CPUs.

Pentium manual section 24.3 (printed 24-3, PDF page 610) charges one clock
per prefix. P5's reciprocal estimate includes a 66h prefix for every dword
operation and reserved copy. Division by seven still wins that static
comparison, 36 versus IDIV's 46 without setup. It no longer wins on 486
using an arbitrary midpoint. P5 is the currently exercised reciprocal path;
all three LNGMXX compiler variants pass actual-program checks.

The multiply-chain selector's broader profiles still use approximate costs.
Neither that nor the division comparison includes a post-allocation spill
cost or complete scheduling model. Further optimization must not describe
these rankings as measured execution time.

## Local GCC cross-check

Inspected revision `9a135e85c2e6543031657ce637e22e1eab004493` in
`/Users/alim/work/other/gcc`, both `gcc/config/i386/x86-tune-costs.h`
and its consumer `gcc/config/i386/i386.cc`. The latter's integer `MULT`
case counts **set bits** of a constant, then adds `nbits * mult_bit` to
`mult_init`; an unknown multiplier uses an explicitly arbitrary seven.
This is not the 386 manual's logarithmic early-out formula. For example,
positive imm8 64 and 85 have different population counts but both take
13 core clocks under that formula. Do not replace the audited immediate
formula with GCC's heuristic and label the result clocks.

The P5 table assigns multiply 11 and divide 25 `COSTS_N_INSNS` units;
the audited dword register forms are 10 and 46 core clocks respectively.
P6 uses multiply 4 and divide 17 units. These tables are useful tuning
references, not independent measurements of our operand forms, prefixes,
or schedules. The 486 per-bit field is even stored as raw `1`, while its
multiply startup is wrapped in `COSTS_N_INSNS`: copying printed numbers
without reading their consumer would also lose the unit scale.

This cross-check changes no selection or emitted assembly. It establishes
that compiler heuristics and hardware timing evidence must stay separately
identified; neither validates the other's numbers by resemblance alone.

## Complete 386 static-ranking forms

The shared 386 profile now covers the ordinary integer operand forms emitted
by the C frontend. Integer register/memory transfer costs are cross-checked
against Open Watcom's `bld/cg/intel/c/x86regsv.c`: for 386 it uses 4 for a
load, 2 for a store, 2 for a push and 4 for a pop. Its `x86mul.c` independently
uses 2 for integer add and 3 for a constant shift. Memory arithmetic in the
llrm profile composes those units rather than treating a memory operand as a
register operand.

POP to memory is not that register POP form. Intel's 80386 instruction table
gives POP m16/m32 five clocks, and the Intel486 Programmer's Reference Manual
gives POP m16/m32 six clocks versus four for POP r16/r32. The later profile
rankings follow the checked-in GCC machine descriptions: `pentium.md`,
`ppro.md`, `k6.md`, `athlon.md`, and `core2.md`. GCC has no distinct K5
scheduler entry, so K5 conservatively uses K6's three-cycle form. LLVM names
the general structural concern as `TuningSlowTwoMemOps`: CALL, PUSH, and POP
with memory operands should be unfolded through a register on affected CPUs.
`cycles.classify` consequently reports `pop_m` rather than silently charging
the cheaper `pop_r` row.

The x87 ranking uses GCC's `i386_cost` in
`gcc/config/i386/x86-tune-costs.h`: 8-unit floating loads/stores and
23/27/88-unit add/multiply/divide. A memory arithmetic form includes the
corresponding 8-unit load. These are compiler tuning units, not measured core
clocks; the report continues to describe them as weighted cost.

`cycles.classify` now names x87 loads, stores, arithmetic, conversions and
control-word transfers instead of collapsing all of them to `unknown`. A form
with no entry returns a null weighted cost. In particular, non-386 profiles do
not inherit the old generic two-unit fallback for an unclassified instruction,
and `rep` remains unpriced until its runtime count is known.

## Complete cross-profile x87 report coverage

The 486, P5, P6, K6, K7 and Core profile columns now cover every x87 form
currently emitted by the C frontend. The arithmetic rankings are taken from
the corresponding local GCC tuning tables in
`gcc/config/i386/x86-tune-costs.h`; floating load/store and conversion entries
come from the same processor-cost structures. A memory arithmetic form is
recorded separately from a register-stack form. Where GCC exposes arithmetic
and load separately, the report composes those two explicitly rather than
silently treating memory as a register operand.

AMD publication 20007D, *AMD-K5 Processor Software Development Guide*, table
2-3, gives the K5 double-real forms used here: FLD 6, FSTP 6, FADD 7 and FMUL
10 for memory operands, and FADD 5, FMUL 8 and FISTP int64 7 for stack/integer
forms. That table omits FDIV even though the instruction is supported. The K5
FDIV value therefore remains a clearly provisional conservative ranking copied
from K6; it is not an audited K5 latency and may not be used as a hard target.

`LEAVE` is ranked as the profile's frame-register move plus pop. Control-word
loads and stores temporarily use the profile's floating memory-transfer rank.
Those are explicit, reviewable approximations and not primary timing claims.
The quality report calls every one of these values `weighted_cost`, never
measured cycles.

The scorer now represents the otherwise implicit x87 accumulator stack as a
dependency. Before this correction, P6 could score a dependent
FLD/FADDP/FMUL/FDIVP/FISTP chain as only the slowest member. A single synthetic
stack token conservatively serializes that chain. It may overstate overlap
between independent x87 expressions, but it cannot hide dependent work as
parallel work; a future eight-entry stack model can refine it without changing
the cost-table interface.

## Deriving the truncating control word with integer work

C floating-to-integer conversion previously initialized both control-word
slots with `fnstcw`, then ORed the truncation bits into the second slot. The
second x87 state store is unnecessary: one `fnstcw`, followed by an integer
load/OR/store, derives the same word while retaining the original for the
restore. GCC's local i686 compiler emits the same dependency shape for the C
floats benchmark: `fnstcw`, `movw`, `orb`, `movw`.

On the committed benchmark the emitted setup changes from 11 bytes and three
instructions to 12 bytes and four instructions. That one-byte static tradeoff
removes an x87 control-store operation. The form-specific quality ranking falls
on every profile: 363 to 355 on 386, 281 to 273 on 486, 111 to 107 on P5, 138
to 137 on P6, 145 to 141 on K5, 136 to 134 on K6, 111 to 109 on K7, and 135 to
133 on Core. These are weighted static costs, not elapsed cycles. In particular,
the older standalone cycle scorer has no memory dependency model and may place
the old `fnstcw [slot]` and following `or [slot]` in parallel; that result is an
instrument limitation, not evidence that the dependency is absent.
