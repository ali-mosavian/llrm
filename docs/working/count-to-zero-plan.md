# Count-to-zero cost term: plan (2026-10-29)

Rows: AM-CZ-1 (x_bubble m32), AM-CZ-2 (PLASMA m16). Oversight's form: a general LSR cost term read from the target cost table, never a loop-shape match; numbered DEVIATE with permalinks to the pin's LSR cost code it extends.

## Findings
- The pin's `loop-reduce` and ours give the same IR on both loops; main's gcc-style pass was better on them. The gap is a term the pin does not have.
- x_bubble: the zero-end formula needs a counter starting at minus the variable bound, shared with the address. Neither the pin's nor our generator makes it (ICmpZero formulas are `bound - i` and scale -1). A cost term alone changes nothing: the trial left x_bubble at 88533 clocks. It needs a formula generator addition first.
- PLASMA: forcing zero-end ICmpZero formulas gives main's loop (no `cmp`). The model ties (4 vs 4) because the unfolded offset on a Basic use is charged a base add that an x86 `lea` absorbs.

## Trial (local, never pushed): the `lea` absorbs the unfolded offset of a non-address use with at most two registers
Rows (m32, tools/rows/run.sh, same binary, env switch on/off, 284 programs x4 levels): 85 rows change.
- better: ring -9.8%, x_cordic -4.4%, histo -2.7%, mandel -1.3%, quicksort -0.5%, x_crc32 -0.5%, scroll -0.4%.
- worse: x_base64 +7.2..7.8%, x_rle +1.9..3.6%, shellsort +2.9% (-O1/-O2), +0.6% (-O3), bintree -O1 +3.0%, recgcd -O1 +2.2%.
- QCport (tools/rows/qcport.py, no flags): 144 of 520 module compiles change, -119..+83 B; totals +119 B (-Os), +536 B (-O1), +367 B (-O2), +204 B (-O3).
Verdict: as a flat term it is not a win. It helps loops whose offset really folds into an `lea` and hurts ones where the offset costs an add.

## Plan
1. Price the add absorbed by the addressing form from the target table (`lea` vs `add`), per use kind, not a flat discount (owner: laa-runtime-checks-loadelim; by 11-04).
2. x_bubble: the formula family (start `-bound`, shared with the address) in the generator, as a numbered DEVIATE; the pin has no counterpart (by 11-06).
3. Each step: fail-first minimal .ll, rows pair against main and parity, QCport, compile Ir.

## x_bubble: the formula generator (plan, 2026-10-22; code 11-06)
Pin evidence (`opt-dbg` with the formula trace, pin.py target m32, on the pre-LSR IR 30-lcssa.ll of x_bubble): for the inner loop's ICmpZero use the pin generates only `reg({bound,+,-1})` (counts down: needs a second, ascending IV for the address), and `reg({bound}) + -1*reg({0,+,1})` / `reg({bound}) + -1*reg({1,+,1})` (scale -1 on the shared IV). It chooses the last, with the address `@_a + 4*reg({0,+,1})`: one register, `inc; cmp; jne`. No formula `reg({-bound,+,1})` (start minus the bound, ascending to 0) exists in the pin's set, so the pin cannot make main's loop (`@_a + 4*bound` as the address base, the counter as its index, `add 4; je`).
What main's form needs: for an ICmpZero use `bound - iv` and an Address use `a + 4*iv`, the formula pair {ICmpZero: `reg({-bound,+,1})`, Address: `reg(a + 4*bound) + 4*reg({-bound,+,1})`}: the counter is shared, its end is zero, and the address base is loop-invariant in the inner loop.
Plan: a generator step after `GenerateScales` (a numbered DEVIATE, no counterpart in the pin): when an ICmpZero formula holds `reg(B) + -1*reg(IV)` with B loop-invariant, add the formula `reg(-B + IV)` (zero-end), and for each Address use of the same IV add `reg(base + stride*B) + stride*reg(-B + IV)`. The cost term decides (zero-end saves the compare: LU2 and `has_zero_end`); no loop-shape match. Tests: fail-first minimal .ll of the bubble inner loop (the chosen ICmpZero formula is zero-end, one IV); rows x_bubble, queens-style loops, quicksort; QCport; compile Ir.
