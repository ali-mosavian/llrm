---
architecture-index: optimizer pipeline design
architecture-order: 100
---
# The optimizer pipeline: what it costs and a structure that costs less

A design note, with the measurements it rests on. Nothing here is built yet.

TL;DR: the MIR pipeline is 21% of qbq-llrm's `d_faces.c` compile at -O1 (1964 M of 9541 M instructions). Its rounds after the second buy nothing and cost a third of its work, and 59% of its work is passes that changed nothing. A fixed order with a second, worklist-driven round saves about 10% of the compile. The 15x gap to gcc and clang is not here: it is the allocator's steps that grow as N^2 to N^3 (last section).

## What runs, and how often

`d_faces.c` `-m32 -O1`, 11 functions, `LLRM_DEBUG=runs`. A round runs every pass in order; the loop stops when a round changes nothing (`pipeline::Fixed::fixed`, at most `max(16, size + 1)` rounds). Each round also renders the whole body to text to catch a cycle.

| round | pass runs | runs that changed | work (M) | share | of it idle (M) |
|---|---|---|---|---|---|
| 1 | 231 | 46 | 884 | 45% | 189 |
| 2 | 220 | 32 | 417 | 21% | 220 |
| 3 | 196 | 5 | 285 | 15% | 170 |
| 4 | 21 | 2 | 210 | 11% | 164 |
| 5 | 18 | 0 | 168 | 9% | 168 |

Per pass (work includes the analyses it asked for first):

| pass | runs | changed | work (M) | own (M) |
|---|---|---|---|---|
| fold | 36 | 12 | 399 | 17 |
| hoist | 35 | 5 | 353 | 195 |
| gvn | 35 | 12 | 270 | 193 |
| decide | 35 | 3 | 232 | 165 |
| dse | 35 | 12 | 149 | 115 |
| promote | 35 | 6 | 122 | 55 |
| algebraic | 35 | 6 | 104 | 71 |
| indvars | 35 | 0 | 102 | 79 |
| sroa | 11 | 0 | 71 | 4 |
| loopmotion | 35 | 2 | 62 | 9 |
| unroll, fill | 25 | 0 | 41 | 24 |

gcc runs each pass once in `passes.def` order, a few twice; LLVM's function pipeline likewise (`PassBuilder::buildFunctionSimplificationPipeline`), each pass once, InstCombine, SimplifyCFG and GVN at fixed places.

## What a rerun buys

Capping the rounds (`LLRM_MAX_ROUNDS`, measured, not kept), executed work (instructions + memory operands, `LLRM_DEBUG=cost`) over the 89 bench and x_ programs at -O1:

| rounds | geomean work | programs worse | worst |
|---|---|---|---|
| unlimited | 1.0000 | | |
| 2 | 1.0016 | 1 | sieve +15% |
| 1 | 1.4257 | 73 | scroll 76x |

`d_faces.c` -O1: 2 rounds produce the listing of unlimited rounds (2148 against 2152 lines) for 8860 M against 9544 M instructions (-7%). One round leaves 2527 lines. Round 2 is the clean-up the first round exposes; rounds 3 and later are verification.

### The rounds are bounded

*Removed: the passes now run once each in LLVM's order (`Options::rounds`, `LLRM_CHECK_ROUNDS` and the in-run skip are gone). The measurements below are why the rounds were bounded while they lasted.*

`Options::rounds` is 4 at -O1, -O2, -O3, -Os and -Og; -Omax runs to the fixed point. Measured with the budget as a cap (`LLRM_MAX_ROUNDS`, a temporary knob), the 71 programs of the vsgcc tables at the four levels (ins, mem and code of each row, 284 rows):

| rounds | rows that differ | worst |
|---|---|---|
| 3 | 1 of 284 (sieve -Os code -1 B) | none worse |
| 2 | 12 to 14 of 71 at -O2, -O3 and -Os; sieve at -O1 | rectwo ins x2.33, recsum -Os x3.16, recfloat x1.46, recgcd x1.36 |

Round 3 matters (the recursion programs); round 4 matters to the bench (3 rounds: jpeg nib -Os +78 B, grep c -Os +4 B, jpeg bq32 +2,000 instructions of 8.4e7), and rounds 5 and later did not on any row of the vsgcc tables or the bench (424 measurements, 0 problems at 4). QCport and the 66 programs at -O1, -O2 and -Os: 15 of 408 objects differ and none by more than 25 bytes (net -24 B at -O2, +22 B at -Os). d_faces default flags, compile instructions with 4 rounds: -O1 6852 M to 6500 M (-5.1%), -O2 8321 M to 7791 M (-6.4%), -O3 9099 M to 8566 M (-5.9%), -Os 8230 M to 7695 M (-6.5%); the 136-file corpus -0.6% to -1.5%.

### What a pass's reads can and cannot say

A footprint says what an instruction is, not where the value it defines sits: a moved load is `MEMORY` alone, while it changes what `lcssa` (and a loop bound that becomes invariant, and a speculation a `phiopt` may do) sees. So a pass that reads where values are defined cannot leave `MEMORY` out of its reads; `lcssa` skipped a run it needed at x_nbody_f -Omax, found by the check when its reads were narrowed. The footprint of the edits since an analysis's result was dropped had no memory and no structure in 19% of the recomputations that came out equal, and none that came out different had; narrowing an analysis by it is safe and worth about 70 M of d_faces -O1.

## Complexity by step

Slopes from `scaling.py run`, `-O2` own Minstr, top four sizes; llrm against gcc at -O1, same generated programs. The exponent is cost against N.

| axis | llrm slope | gcc slope | llrm/gcc at -O1, smallest N | largest N |
|---|---|---|---|---|
| straight | 0.98 | 0.96 | 0.83x | 1.12x |
| functions | 1.01 | 1.01 | 2.2x | 2.7x |
| chain | 1.06 | 0.97 | 1.8x | 5.2x |
| live | 1.42 | 1.47 | 2.4x | 3.5x |
| cells | 1.59 | 1.42 | 1.8x | 4.9x |
| joins | 1.81 | 1.41 | 2.8x | 7.1x |
| branches | 1.25 | 1.00 | 2.9x | 9.0x |
| nest | 1.66 | 1.73 | 4.9x | 4.9x |
| unrolled | 1.80 | 1.04 | 9.0x | 49x |

Steps above exponent 1.15 (axis: slope), optimizer first:

| step | where | slope |
|---|---|---|
| mir lsr | nest, unrolled | 2.2 |
| analysis counted | nest | 2.6 |
| analysis loop-products | unrolled | 2.4 |
| mir dse | unrolled | 2.0 |
| mir unroll | branches | 2.0 |
| mir hoist | nest | 2.0 |
| mir indvars | branches, nest | 1.6, 1.9 |
| mir gvn | branches, joins, unrolled | 1.2, 1.8, 1.7 |
| mir decide | branches | 1.2 |

Allocator and LIR steps, the larger part:

| step | where | slope |
|---|---|---|
| intervals homes | branches, nest, joins | 2.1, 2.7, 3.2 |
| spill cleanup, spill stable loads, spill color slots | cells, live | 2.0 - 2.2 |
| regalloc candidates, trial | branches, nest, unrolled | 1.9 - 2.8 |
| lir coalesce | cells, live, joins | 1.5 - 2.5 |
| lir phielim | branches | 1.9 |
| facts intervals, widths | live | 1.8 - 1.9 |

## Structure

1. **A fixed order, run twice.** Round 1 is the pipeline in order. Round 2 runs a pass only on what round 1's edits reached. Nothing after round 2. LLVM's pipeline and gcc's `passes.def` are fixed sequences; the second round stands where InstCombine/SimplifyCFG are repeated after GVN and LICM.
2. **A worklist per pass in round 2.** An edit log already exists (`Function::changes_since`, `footprint::of`); a pass in round 2 takes the changed blocks and the loops containing them, not the function (LLVM's InstCombine worklist, LoopPassManager's loop queue, gcc's `TODO_cleanup_cfg` flags). A rerun costs O(changed), not O(N).
3. **A necessary condition before a pass's analyses.** `indvars` changed nothing in 35 runs, `sroa` in 11, `unroll` and `loopidioms` in 25, `floatloop` in 35 (gated in #1708): 102 + 71 + 41 + 233 M. Each asks whether its shape exists before it asks the analyses (LLVM passes bail out early on inputs they cannot change).
4. **Analyses maintained or lazy, not recomputed whole.** `points_to` (7.3% of the compile), memory SSA `Accesses` (5.6%) and `Annotated` (5.3%) are recomputed after a change. [Points-to across edits](points-to-lifetime.md) is the maintained form; LLVM updates MemorySSA with `MemorySSAUpdater` and preserves AA across passes.
5. **Budgets above O(N log N).** A pass whose slope is above 1.15 on an axis takes a limit in the style of LLVM's `MemorySSAScanLimit` / gcc's `--param`: #1486 and #1487 did it for loads reached by more than 100 writes. `lsr`, `dse` on unrolled, `counted` and `loop-products` are the candidates in the table above.
6. **No whole-body text per round.** The cycle check renders the function to a string every round; with two rounds it is not needed.

## Expected effect

On `d_faces.c` -O1: rounds 3+ and the body renders about 7%; idle reruns of passes the worklist skips and the gates in 3 about 5%. Together about 10% of the compile, with the 89-program work within +0.2% (the 2-round row above). This is the optimizer's whole share: even free, it leaves 8.5 G against gcc's 0.64 G.

## Where the 15x is

The compile at -O1 is 62% backend and 48% the register allocator. The hot function (1888 instructions, 160 spilled values) is allocated up to six times (base, two trials, two routes) at about 20 µs per instruction, and the steps in the second table grow as N^2 to N^3. That is the regalloc session's work (incremental rounds, one allocation); this note does not cover it.
