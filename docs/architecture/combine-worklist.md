# One combine pass with a worklist (plan step 3.2)

TL;DR: `fold`, `algebraic`, `phiopt`, `dead` and `decide`'s branch folding become one pass that visits each instruction once and queues the users and operands of every change, as LLVM's InstCombine does. The rewriting is not what costs: on d_faces -O2 `fold` and `algebraic` spend 330 M against InstCombine's 39.5 M, 290 M of it on function-wide facts that 94% of the edits do not need. A register-only worklist plus a six-instruction load scan makes 78.8% of `fold`'s number edits at a tenth of the cost, and the other 21% are loads of stack slots that `promote` removes if it runs first. So the order is: promote before fold, then the worklist.

## The invariant

No round loop; an explicit list of positions; each position runs to a fixpoint on one worklist, queueing from the edit log (`Touched`, #2071 and #2075). This is LLVM's: [InstCombine](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp) runs about four times in the function pipeline and again after the vectorizer (PassBuilderPipelines.cpp), because GVN, the loop passes and the vectorizer make work for it; what it does not do is come back to itself inside a position. `LastRunTracking` is `settled` and `reads()`.

## Measured, in the order the decision needs

All counts are deterministic (instructions, edits) from the shadow instrument: at each `fold` run it computes what a bare-unit register worklist (`consts::known_from` seeded with every value, plus the table loads) would have replaced, and logs it; nothing changes. The corpus is the gate's: 1,583 objects × 4 levels.

| What | Measurement | Reading |
|---|---|---|
| `fold`'s number step replaced by the worklist, base order | 35,609 fold runs, 63,821 values: worklist 77.1%, six-instruction store scan 1.7%, missed 21%; POC-only edits 0 | the slots are still memory when `fold` runs |
| Same, on the order experiment (sroa+promote early, `alim/spike/sroa-poc`) | 60,763 runs, 38,978 values: 85.8% hit, missed **14.2%**; 1,496 of the misses are loads of globals and 4,700 of the 5,528 depend on them | a defect of the worklist, not of memory: it dropped a pointer-valued user (a `getelementptr` whose index just became a number), so the load through it was never asked again |
| Same, with `known_from` queueing the users of a pointer-valued instruction | missed **2.9%** (1,143): loads of allocas 392, of other pointers 200, of globals 9, arguments 8, phi 2, the rest computed from those | slots the order experiment still leaves: the SROA unit's job |
| Cost of the step | corpus: `fold`'s number step 20,775 M, the worklist 6,116 M (3.4x less); d_faces -O2 under the order: 14.5 M against 93.2 M (6.4x), 0.20 M a run against InstCombine's 0.45 M | per IR instruction the worklist is 940-1,290 instructions against InstCombine's ~430: a fifth of it is the frame-exposure table and a tenth the table-load list, which InstCombine has no equivalent of (loads of allocas are SROA's before it runs) |
| `floatfold` | 29 changing runs of 2,274, 467 M on 43 compiles, d_faces -O2 126.9 M for one edit that changes 915 lines | DEVIATE with a number; not the combine's |
| `algebraic`'s edits | 12.8 K over the corpus: 718 (5.7%) ask a function-wide fact, the rest are local patterns, 44% of them `_nonnegative_sext` over the depth-limited `known_zero` | per-query facts, not solves; on d_faces 73 M of set-up serves those 5.7% |
| Who made what `algebraic` rewrites | the C front end made 80% of the instructions it rewrites (8,044 of 10,033), Basic 40%, Nib 49%; the passes' clones (unroll, peel) inherit them | the representation is fixed where instructions are made |
| Constant folding at creation | `Builder` folds numbers (translation of ConstantFold.cpp): optimized outputs identical, d_faces -0.3% dist, lru -0.9% | first step of the representation lane |
| Repeats of the members | run once in round 1, all four together, 1,447 objects: 168 differ (11.6%), d_faces -7.0% at -O2; one at a time they sum to 202 | the joint figure; the sum is not a bound |
| Idle repeats | 213 traces: fold 1,056 reruns after round 1, 27 changed; algebraic 972 / 21; decide 1,056 / 44; phiopt 994 / 0 | 96% to 100% idle |

The enablers of the changed reruns (dse, loopmotion, lcssa, ...) were counted as passes that changed the body since the member's last run. That is co-occurrence: the same 22 events appear under four passes. It does not show cause and places nothing.

## The instrument lesson

The first validation (d_faces and 37 benches) said 0.5% missed; the replacement built on it differed on 36 of 1,583 objects; the corpus said 21%. Then the corpus on the order experiment said 14.2% and the raw dump of one program (`mul_constants`) showed a defect in the worklist that explained 11 of those points. A validation set is the set of inputs it was taken from: the gate's corpus is the set for anything the gate judges, and a miss class is read in a raw dump before it is explained.

## What is translated

The worklist fold follows [`InstCombinerImpl::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5780) and [`prepareWorklist`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L6031):

| C++ | Here |
|---|---|
| `prepareWorklist`: reverse post-order seed, constant fold and delete the dead as it goes, only the live successor of a constant branch | MIRROR to write. The POC seeds every value in `known_from`'s order and skips none |
| `run`: pop deferred, delete the trivially dead (and its operands), pop one, visit | MIRROR to write: the existing `dead` rule is the deletion; `visit` is the rule set |
| after a change: `pushUsersToWorkList(*Result)`, push the result, `eraseInstFromFunction` | MIRROR to write. The POC queues users of a known value and, after the fix, of a pointer-valued instruction |
| `visit` constant folds through `simplifyInstruction`, the same `ConstantFold` as the builder | `constfold.rs` is that translation; `simplify` over operands is next |
| replacement per visit (`replaceAllUsesWith` at once) | DEVIATE in the POC: it keeps a fact map and replaces after the fixed point. The translation replaces at the visit |
| `LastRunTracking`, `MaxIterations`, sinking, `AliasScopeTracker` | `settled` and `reads()`; the rest not translated: no code sinking in this pass, no alias scopes in MIR |
| `isInstructionTriviallyDead`, `ConstantFoldInstruction` | `dead::_kept`/`_removable`, `constfold` |

## Design

Three changes that must be priced apart (one measurement per change, each from its parent):
1. The merge: one worklist pass over the existing rules. By the numbers above it is near zero on its own.
2. Facts per query: `fold`'s number step as the register worklist (needs the slots gone); `algebraic`'s facts asked per candidate (#2086 is the first); `floatfold` as a DEVIATE until a per-query form exists.
3. Position: after the loop group and `dse`, the places the enablers sit; rounds drop only after the above.

The representation lane is the larger lever for the width rules: LLVM's `IRBuilder` folds at creation (`IRBuilderFolder`, `InstSimplifyFolder`), so clang emits few redundant casts. Ours has one construction point (`llrm_mir::build::Builder`, used by the front ends and lowering). One shared `simplify` over (opcode, operands) called by the builder, the combine and `gvn` is InstSimplify's shape; a rule per creator is not.

## Kill

- A joint run-once of the members that changes more than 25% of objects, or any row worse that the repeats held.
- The worklist fold still missing more than 2% of today's number edits after the SROA unit lands (2.9% now on the experiment).
- No compile-time exit: the plan's -8% at -O2/-Os is the number to meet.

## Risks

- Downstream passes see unfolded IR if a position moves; identical.py on the 1,583 objects before any move.
- Rules that undo each other's canonical forms: a visit cap and `LLRM_CHECK_FIXPOINT`.
- `jumpthread`'s sandwich, CFG edits inside the worklist (InstCombine leaves the CFG alone), -O1/-Os/-O3 and m16 differences.
- The slice must run on all 1,447 objects, not the ones an ablation picked.
