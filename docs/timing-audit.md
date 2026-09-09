# Arithmetic timing audit — incomplete

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

`backend/timing.py` now separates audited multiplication ranges and division
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
