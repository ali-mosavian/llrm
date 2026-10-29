---
architecture-index: the inliner as LLVM's: translation plan
architecture-order: 450
---
# The inliner as LLVM's: translation plan

TL;DR: translate `InlineCost.cpp` (the cost model and thresholds) and `InlineAdvisor.cpp::shouldInline`
first, then the bottom-up walk (`Inliner.cpp::InlinerPass::run`, `buildInlinerPipeline`), behind
`LLRM_EXP_LLVM` on the order branch. Inline decisions change output, so they are LLVM's or listed as DEVIATE
with a number. Order: (1) the per-site decision diff against `opt -passes=inline`, (2) the cost model,
(3) the walk.

Sources: LLVM at [d1106deb71cc](https://github.com/llvm/llvm-project/tree/d1106deb71cc) (links below are under
`llvm/lib/`). Rule: the Rust follows the C++ (functions, worklists, thresholds with the `cl::opt` default, order
of checks), one permalink comment per function, DEVIATE where it does not.

## What llrm does today

`inline.rs` (`candidates`, `constant_sites`, `fits`, `Plan`): a callee is a candidate when it can be cloned and
its semantic operations fit `max(6, min(24, call_cost / 2)) * limit / 225`, with gcc-shaped growth limits
(caller growth, unit growth 40%, frame 256 B, work copied against calls removed in clocks, or bytes at `-Os`).
`Interprocedural` runs it once every body has reached its fixed point, then sends each changed body back through
the pipeline (`LLRM_DEBUG=inline` logs a verdict per callee). There is no per-instruction cost, call penalty,
SROA or constant-argument bonus, dead block at the site, or `shouldBeDeferred`; the decision is per callee and
then per caller by trial, LLVM's per call site.

On the order branch (`LLRM_EXP_LLVM`) `recorded()` runs `Phase::Early`, `Phase::Cleanup` and `Phase::Simplify`
on every body before `Interprocedural`, so the inline stage takes bodies simplified once with no callee inlined.
LLVM's inliner takes the module after the early function pipeline, GlobalOpt and global cleanup (`Early` and
`Cleanup`), and runs the function-simplification list afterwards, per SCC, callee first, inside the inliner.
The dump after `Cleanup` (`--dump DIR`: the second of the three consecutive `NN-pipeline.ll` files) is the
module before the inline decision; `NN-interprocedural.ll` is after it.

## The map

| Unit | LLVM | Rust target |
|---|---|---|
| Parameters, `cl::opt` defaults (225, hint 325, cold 45, locally-hot 525, hot 3000, instruction 5, call penalty 25) | [InlineCost.cpp:56-186](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L56-L186); `getInlineParams` [3376-3486](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L3376-L3486) | `inlinecost.rs`: `InlineParams` and consts named as the options |
| Callee attribute decision | `getAttributeBasedInliningDecision` [3191-3263](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L3191-L3263); `functionsHaveCompatibleAttributes` [3089](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L3089-L3101) | `attribute_decision` |
| Entry points | `getInlineCost` [3265-3318](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L3265-L3318); `isInlineViable` [3320-3375](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L3320-L3375) | `inline_cost`, `inline_viable` |
| Analyzer state, SROA and offset tables, dead blocks | `CallAnalyzer` [211-600](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L211-L600); `analyze` [2916-3060](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2916-L3060); `analyzeBlock` [2745-2875](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2745-L2875); `findDeadBlocks` [2876-2915](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2876-L2915) | `CallAnalyzer`, `analyze`, `analyze_block`, `find_dead_blocks` |
| Visitors | `visitAlloca` [1575](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L1575-L1617), `visitPHI` [1618](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L1618-L1713), `visitGetElementPtr` [1732](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L1732-L1763), casts [1867-1969](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L1867-L1969), `visitCmpInst` [2206](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2206-L2261), `visitSub`/`visitBinaryOperator` [2262-2323](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2262-L2323), `visitLoad`/`visitStore` [2343-2377](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2343-L2377), `visitCallBase` [2473-2564](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2473-L2564) with `simplifyCallSite` [2411](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2411-L2436) and `isLoweredToCall` [2437](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2437-L2472), terminators [2565-2720](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2565-L2720), `visitInstruction` [2722](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2722-L2744) | `visit_*` over MIR opcodes |
| Cost analyzer: costs, thresholds, bonuses | `InlineCostCallAnalyzer` [604-1250](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L604-L1250): `updateThreshold` [2073-2205](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2073-L2205), `isColdCallSite` [2021](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L2021-L2043), `finalizeAnalysis` [1080](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L1080-L1137), cost-benefit [899-1079](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L899-L1079) | `InlineCostCallAnalyzer` |
| The decision | `getDefaultInlineAdvice` [InlineAdvisor.cpp:144](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineAdvisor.cpp#L144-L182), `shouldBeDeferred` [284](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineAdvisor.cpp#L284-L404), `shouldInline` [412](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineAdvisor.cpp#L412-L470) | `should_inline`, `should_be_deferred` |
| The walk | `InlinerPass::run` [Inliner.cpp:200-616](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/Inliner.cpp#L200-L616), `ModuleInlinerWrapperPass::run` [617](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/Inliner.cpp#L617-L655); `buildInlinerPipeline` [PassBuilderPipelines.cpp:973](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L973-L1073), `buildModuleInlinerPipeline` [1074](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L1074-L1122), `buildModuleSimplificationPipeline` [1123](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L1123-L1517) | `cgscc.rs`, after the cost model |

Defaults: threshold 225 at -O2 and 250 at -O3 (`getInlineParamsFromOptLevel`, [3435](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L3435-L3445); the
pipeline passes the speedup level, so -Os and -Oz use 225 and the size effect is the `optsize` and `minsize`
function attributes clang sets, held to `OptSizeThreshold` 50 and `OptMinSizeThreshold` 5 in `updateThreshold`);
hint 325; locally hot 525; the cost-benefit analysis is off (`inline-enable-cost-benefit-analysis` false), so
`finalizeAnalysis` decides by cost against threshold.

## Steps

1. The decision diff. On programs that inline (the bench programs with their flags, lru, bintree, QCport at
   -O2 and -O3; not d_faces, which is built `-fno-inline-functions`): take the module before the inline stage
   (the dump after `Cleanup`), convert it to LLVM IR as `tools/difftest` does, run
   `opt -passes=inline -pass-remarks=inline -pass-remarks-missed=inline -pass-remarks-analysis=inline` (cost and
   threshold per site), and read what llrm inlined from the dump after the stage. Join by (caller, callee,
   ordinal). Output: both verdicts per site, classes of disagreement. It is the oracle of step 2 and the baseline
   of the rows.
2. The cost model, shadow first: LLVM's cost and threshold per site printed beside the existing decision, nothing
   changed. Then the decision behind `LLRM_EXP_LLVM`; rows decide.
3. The walk: bottom-up SCC order, the function-simplification list on each callee before it is inlined and again on
   the changed caller (`devirt<4>`). The order probe is being re-run on inlining programs; this step waits for it.

## DEVIATE list (each gets a reason and a number when measured)

- D1 Frequencies: LLVM's `BlockFrequencyInfo` decides cold and hot call sites; ours are `profit` block frequencies.
  Thresholds as LLVM's.
- D2 TTI: `getInstructionCost`, `isLoweredToCall`, FP and intrinsic costs come from LLVM's TTI; llrm has operation
  costs per target (`OperationCosts`), used where LLVM asks TTI; `InstrCost` 5 otherwise.
- D3 Opcodes MIR lacks or spells differently (constant GEP offsets, `ptrtoint` pairs, far pointers, x87): the
  visitor falls to `visitInstruction` (one `InstrCost`) until measured.
- D4 PGO, ThinLTO, the ML advisor, `optnone`, `flatten`, coroutines, strict-FP and target-feature checks: no source
  of them in llrm; none.
- D5 The SROA bonus (`handleSROA`, `onInitializeSROAArg`) needs alloca-derived arguments: translated, tested on
  dumps.
- D6 The gcc growth limits (caller growth, unit growth, frame) stay on the gcc path. On sites `inline_cost`
  decided, caller growth (#2622) and the frame limit (#2656, once copies carry lifetime markers) are off.
- D7 `shouldBeDeferred` needs the other callers of the callee with their costs: translated over the module call
  graph the interprocedural stage holds.

- D9 `noipa` (GCC's attribute; LLVM has no IR equivalent: `optnone`/`noinline` keep a function from being changed
  or inlined, not from being read by its callers' passes). A bench kernel is a real procedure its callers cannot
  fold away. Scope: functions the source marks `noipa`, nothing else; no `-O` flag sets it.
- D10 `InsertLifetime` skips an alloca whose address, or one derived from it, is returned or stored as a value; LLVM
  marks every static alloca, as C makes such code undefined. QB's runtime bodies return their own allocas
  (`B$FMID`'s view): the end at the return made the read through it undef (#2625, SUM= 0). Exit: the lifetime ends
  after the copy's last use of the address, not at its return.

## INLINEDECIDE: what turning it on does (#2140, off by default)

`LLRM_EXP_INLINEDECIDE=1` decides each call site by [`inline_cost`](https://github.com/ali-mosavian/llrm/blob/55ecae453e40/crates/opt/llrm-transforms/src/inlinecost.rs)
(the translation of InlineCost.cpp); `LLRM_EXP_INLINECOST=1` prints the cost beside the old decision. Both are off, and
a build without them is byte-identical to main (1,583 objects of `tools/identical.py`).

Compile time, instructions (`perf stat instructions:u`, `llrm-cc -O2`, QCport modules, base against the switch on):

| file | base | decide | | per-body pipeline re-runs after a splice / fixed-point steps (base -> decide) |
|---|--:|--:|--:|---|
| cmd.c | 1,787.8M | 2,668.1M | +49% | 9 / 35 -> 15 / 88 |
| ls.c | 666.1M | 1,245.9M | +87% | 4 / 19 -> 10 / 63 |
| dbg.c | 114.9M | 146.8M | +28% | 0 / 0 -> 2 / 7 |

The cost analysis itself is a small part (`inlinecost::sites`: 20M of the 827M that cmd.c gains); the rest is the
pipeline over larger bodies, about 6 fixed-point steps per re-run. A callees-first walk (branch `feat/inline-walk`,
`LLRM_EXP_INLINEWALK`) visits each function once and gives the same rows: cmd.c 2,554M, ls.c 1,403M, dbg.c 146M
(87, 71 and 7 steps), so the order is not the cost; the single pass-list run of the order work (llvm-order-poc's step 3.2) is, and the walk PR is not opened until it lands.

Rows (71 vsgcc programs, m32, against the pinned references of `rows/main24d`: gcc 13.4.0, clang 24.0.0git
d1106deb71cc): only x_dct and x_heapsort move, at -O2 and -O3. Clocks -25.0% / -27.9% and -15.1%; code bytes +32% on
each at -O2. Rows above min(gcc, clang), clocks 32 -> 31 at -O2; code bytes 29 -> 31 at -O2. The bench.py set: 8
parity programs and jpeg (c) move; jpeg_fixed (c) code grows 3,331 -> 5,471 B at -O2 for no change in time.

Turning it on is a trade of those rows against the compile time above; it is oversight's decision.

## noipa: the contract (D9)

The fact: a `noipa` function is opaque to every interprocedural pass. Its body is still optimised by the per-body
pipeline; what no pass does is use the body for anyone else, or change how it is called.

- Inlining and cloning: never inlined (`inline`, the InlineCost decision), never cloned or specialised (`ipacp`),
  never argument-promoted, dead-argument-pruned or given a new calling convention (`calleepop`, argument promotion).
- Facts read off the body: none. `Stamp` (function-attrs) does not infer attributes for it and callers read only
  the attributes the source declared; `Summaries`, `GlobalsAA`, `CalleeEffects` and `CallRegisters` treat a call of
  it as a call of a declaration with its declared attributes.
- Constants and values from callers (`current_call_constants`, IPSCCP-style propagation) are not pushed into it.
- It is not deleted by `globaldce` while the module names it, and a call of it is never removed as dead unless the
  declared attributes allow it.

Test, fail first: `crc/nib` at -O3, where `bench_crc` is folded into `main` today ("not in the map"), keeps its
procedure and its call; one unit test per honouring pass (a `noipa` callee stays out of the inline candidates, out of
the stamp, out of the summaries, out of the constants pushed in). The bench sources that name the attribute and the
re-blessed rows are llvm-difftest's, in the same PR.

## Measurements before code is long

- Body size at the inline stage's input and at gvn's input, llrm against LLVM, on d_faces -O2 and an inlining
  program (the reference figures at gvn on d_faces -O2 are 997 and 1,385).
- Compile cost per IR instruction of one analysis run, against the budget of 300.
