# Compile time at LLVM's level: the redesign

TL;DR: d_faces -O2 compiles in 3,147 M instructions against clang's 557 M
(5.7x, dist build). Per-pass levers stop at about 2.5-3x. The rest is four
design choices that touch every pass; LLVM makes each of them differently,
and this plan copies LLVM's mechanism for each, in five stages, each
gated by counted budgets and by output rows against min(gcc, clang).

Sources: LLVM at [d1106deb71cc](https://github.com/llvm/llvm-project/tree/d1106deb71cc)
(links `LLVM:path:line` below are under `llvm/`), llrm at
[cc2806006](https://github.com/ali-mosavian/llrm/tree/cc2806006fc4)
(links `llrm:path:line`). Measurements: d_faces -m32, dist profile,
instructions:u, unless stated; per-pass numbers are own time from
`tools/compile-gap.py` (#1923).

## Where the time goes, and what LLVM does instead

| | llrm today | LLVM |
|---|---|---|
| Analyses (23% own at -O2) | rebuilt per function version; SOUND results dropped every round | cached until a pass says it did not preserve them; updated in place by updaters |
| Memory walker + structure (~3.9%) | structure rebuilt per version (54 builds), each query walks (27.7 K instr; 65% repeat an earlier version's question, 95.6% same answer) | clobber stored on the access with an id stamp; O(1) on a hit; updater keeps it valid |
| Pipeline (rounds >= 2 = 35-56% of step work) | whole pass list 3-4 times to a fixed point | each pass once at chosen positions; convergence inside worklist passes; loops and SCCs revisited only when changed |
| Containers (~47% of the compile) | BTree maps/sets, Vec clones, whole-function clones per speculation | intrusive lists, dense ids, flat maps, small inline buffers, bump allocators |
| Pricing (gvn: 61% of gvn) | whole-function pressure forecast per decision | no pressure in the mid-end; local pressure in MachineLICM, scheduler regions, per live range in the allocator |

## Stage R1 — analyses are state, updated by edits

LLVM:
- A result stays cached until a pass's `PreservedAnalyses` drops it
  ([PassManagerImpl.h:56-92](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/IR/PassManagerImpl.h#L56-L92),
  cache at [:131-165](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/IR/PassManagerImpl.h#L131-L165),
  invalidate at [:168-232](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/IR/PassManagerImpl.h#L168-L232)).
  A pass that changed nothing returns `all()`.
- Each result decides its own invalidation and its dependencies
  ([PassManagerInternal.h:232-258](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/IR/PassManagerInternal.h#L232-L258);
  `DominatorTree::invalidate` keeps itself across CFG-preserving passes,
  [Dominators.cpp:71-78](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/Dominators.cpp#L71-L78)).
- Changes are applied by updaters, not recomputation: the dominator tree
  takes batched edge updates and recomputes only past a size threshold
  ([GenericDomTreeConstruction.h:1137-1184](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1137-L1184));
  SCEV forgets only the loop or value that changed
  ([ScalarEvolution.cpp:8676-8727](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/ScalarEvolution.cpp#L8676-L8727));
  loop passes receive DT, LoopInfo, SCEV and MemorySSA once per function
  and must keep them valid
  ([LoopAnalysisManager.h:49-62](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Analysis/LoopAnalysisManager.h#L49-L62),
  [LoopPassManager.cpp:296-340](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LoopPassManager.cpp#L296-L340)).

llrm today: the manager ([llrm:passes.rs:982-1460](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/ir/llrm-mir/src/passes.rs#L982-L1460))
keeps results as INCREMENTAL, SKIPS or SOUND, but `refresh_soft` drops the
SOUND ones at every round start ([pipeline.rs:1021](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/pipeline.rs#L1021)),
`Accesses::update` survives only moves and operand edits
([memoryssa.rs:322](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-analysis/src/memoryssa.rs#L322)),
`Pressure::updated` gives up on any CFG edit
([spill.rs:1468](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/spill.rs#L1468)).
Measured: Annotated.update bailed in 74 of 90 updates; Accesses likewise.

Change:
1. Every analysis gets an updater from the one edit record (`Change`,
   [module.rs:137](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/ir/llrm-mir/src/module.rs#L137)),
   exact, with the SemiNCA rule: past a change count proportional to the
   body, recompute. No SOUND class: an updated result is exact, so
   `refresh_soft` and the order-dependence class (#1909) disappear.
2. The dominator tree, loop forest and frequencies first (everything else
   depends on them), then Registers/ranges, PointerValues, Accesses.
3. Passes declare preserved results the LLVM way; a pass that changed
   nothing preserves all (already: `unless_unchanged`).

Gate: `analysis_computations / pass_runs <= 0.5` (spec §6) as a counted
test; `LLRM_CHECK_REPLAY` and `LLRM_CHECK_ORDER` silent; identical output.
Ceiling, measured on d_faces -O2 (`LLRM_WHY`): of 2,784 analysis computes
911 are first asks and 814 are already updates; 1,059 (at most 40%) are
recomputes after a drop, and a third of those change the answer. A declared
footprint that skips a recompute realises about 0.2%: what blocks the rest is
an erased or rewritten integer load or store (79), module facts that moved
(92), an edit that reaches a modelled instruction (43), blocks made or erased
(31). `LLRM_WHY` names the blocker of each; measure per analysis, and build an
updater only where its blocker is at least 1% of the compile. Owners: keep-analyses (meaning),
edit-rollback (record and updater API).

## Stage R2 — MemorySSA that persists

LLVM:
- The clobber of a use is stored on the access, stamped with the
  defining access's id, so it invalidates itself
  ([MemorySSA.h:329-356](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Analysis/MemorySSA.h#L329-L356)).
  A query returns it in O(1) and caches a miss on the access
  ([MemorySSA.cpp:2556-2591](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSA.cpp#L2556-L2591)).
  The batch form is a dominator-tree walk with a version stack
  ([MemorySSA.cpp:1299-1507](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSA.cpp#L1299-L1507)),
  run only on demand (lazy since D121381).
- `MemorySSAUpdater` keeps it valid across insert, remove, move and CFG
  edits, resetting only the affected uses
  ([MemorySSAUpdater.cpp:353](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSAUpdater.cpp#L353),
  [:422-635](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSAUpdater.cpp#L422-L635),
  [:1262-1285](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1262-L1285),
  [:1401-1466](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1401-L1466),
  CFG at [:899-948](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSAUpdater.cpp#L899-L948)).
  GVN, EarlyCSE, DSE, LICM, MemCpyOpt, loop rotation and unswitch all
  preserve it; inside LPM1 not preserving it is a fatal error.

llrm today: `Structure` is built per body state
([memoryssa.rs:1481-1505](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-analysis/src/memoryssa.rs#L1481-L1505)),
the walker's caches live in a per-call `MemorySSA<'a>`
([:847](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-analysis/src/memoryssa.rs#L847)),
and every client builds one (gvn
[avail.rs:816](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/gvn/avail.rs#L816),
ThroughMemory [consts.rs:1053, 1118, 1260](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-analysis/src/consts.rs#L1253),
loadjoins.rs:294). Measured at -O2: 4,787 queries, 133 M; 54 structure
builds, 71 M.

Change: `Structure` becomes the manager-owned MemorySSA, updated from the
edit record (memory ops inserted/erased/moved/rewritten, CFG edges); the
nearest-writes answer is stored per (access, cell) with a stamp of the
writes it depends on, reset by the updater on the affected chain only;
every client asks this one owner. This replaces the per-version cache
measured and rejected in spec §8 (that cache kept the rebuild; this
removes it).

Gate: `LLRM_CHECK_CLOBBERS` against a fresh walk on every query; identical
output; counted test: structure builds per function <= 1 + CFG-recompute
events. Ceiling ~3.9% at -O2 plus the clients' own re-asks. Owner:
hoist-bounded (did the walker study), with keep-analyses.

## Stage R3 — LLVM's pipeline: placements and local cleanup, no rounds

LLVM:
- The function pipeline runs each pass at fixed positions, with the
  cleanup passes several times: SimplifyCFG and InstCombine at about six
  points, JumpThreading/CVP/SROA twice, LICM three times, DSE after GVN
  ([PassBuilderPipelines.cpp:646-837](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Passes/PassBuilderPipelines.cpp#L646-L837)).
- Convergence is inside worklist passes: InstCombine re-queues users and
  operands on every change and deletes dead code on the spot
  ([InstructionCombining.cpp:5780-5963](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5780-L5963),
  [InstCombineInternal.h:504-520](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineInternal.h#L504-L520)),
  with one outer iteration
  ([InstCombine.h:29](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Transforms/InstCombine/InstCombine.h#L29));
  SimplifyCFG resimplifies per block
  ([SimplifyCFG.cpp:9392-9405](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9392-L9405));
  dead chains go by worklist
  ([Local.cpp:522-592](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/Local.cpp#L522-L592)).
- Loops: a per-loop worklist re-runs only the loop that changed
  ([LoopPassManager.h:259-313](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Transforms/Scalar/LoopPassManager.h#L259-L313)).
- Functions: bottom-up CGSCC order; the only whole-pipeline repeat is on an
  SCC whose indirect call was resolved, at most 4 times, and unchanged
  functions are not re-run
  ([CGSCCPassManager.cpp:451-541](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/CGSCCPassManager.cpp#L451-L541)).

llrm today: one pass list `every`
([pipeline.rs:483-531](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/pipeline.rs#L483-L531)),
`ROUNDS = 4` ([:253](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/pipeline.rs#L253)),
`Fixed::fixed` to a fixed point with the body printed to text each round as
the oscillation guard ([:993-1069](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/pipeline.rs#L993-L1069));
`Interprocedural` reruns `Fixed` ([:774](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/pipeline.rs#L774)).
Measured (hoist-bounded): rounds >= 2 are 35-56% of step work, half of it
idle; the chain that needs round 3 is fold → decide → lcssa → gvn, i.e. the
missing local cleanup after a decided branch.

Change:
1. A combine pass: fold, algebraic, phiopt, dead and the branch-folding
   part of decide as one worklist pass with LLVM's re-queue rule (users and
   operands of every change, trivially-dead deletion on the spot, single-
   entry phi folding when blocks merge). The existing rules move; no new
   rules.
2. Place passes at LLVM's positions (combine and CFG cleanup as the
   sandwich, dse after gvn, indvars in the second loop group before gvn).
3. Loop passes on a per-loop worklist: a pass that changes a loop re-queues
   that loop only.
4. Rounds to 1 at -O1..-O3/-Os; -Omax keeps the fixed point. The
   interprocedural rerun becomes bottom-up order plus rerun of a changed
   callee's callers only. The text-print oscillation guard goes with the
   rounds.

Gate: changes output, so judged by rows: 71 programs + d_faces + QCport,
clocks vs min(gcc, clang), worst row beside geomean, no row newly below.
Steps 1-2 land first at today's round count (output identical or rows
held), then rounds drop. Ceiling: hoist-bounded measured 2 rounds at
-9.8% d_faces compile with the reordering alone; 1 round removes the rest
of the 35-56%. Owners: pass-list-parity (pipeline, combine pass),
hoist-bounded (rounds, loop worklist).

## Stage R4 — IR and container costs

LLVM: intrusive use lists make RAUW O(uses) with no hashing or allocation
([Use.h:82-104](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/IR/Use.h#L82-L104),
[Value.cpp:518-551](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/Value.cpp#L518-L551));
instructions sit in an intrusive list with lazily renumbered order for
O(1) local dominance
([Instruction.cpp:335-343](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/Instruction.cpp#L335-L343),
[BasicBlock.cpp:636-644](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/BasicBlock.cpp#L636-L644));
analyses bump-allocate and free in bulk
([ScalarEvolution.h:2595](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Analysis/ScalarEvolution.h#L2595),
[LiveIntervals.h:67-68](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/CodeGen/LiveIntervals.h#L67-L68));
hot sets are flat or inline (DenseMap, SmallVector, SmallPtrSet, BitVector,
SparseSet).

llrm today: use lists are already stored
([module.rs:515](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/ir/llrm-mir/src/module.rs#L515));
block order is a `Vec<InstId>` (O(block) insert,
[:82-86](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/ir/llrm-mir/src/module.rs#L82-L86));
whole functions are cloned per speculation in about 15 places (hoist.rs:306,
lsr.rs:599, peel.rs:254, unswitch.rs:213, ...); containers are 47% of the
compile (malloc/free 11%, BTree 10%, Vec 9%, hash 8%).

Change: order numbers per block with lazy renumbering (spec stage 4);
checkpoint/rollback (#1937) replaces every speculative clone; dense ids use
bit sets and id-indexed vectors (the ratchet test from #1927 extended to
BTree over dense ids); per-analysis arenas. Gate: identical output;
counted tests `function_clones = 0` and container allocations per pass run.
Ceiling: a share of the 47%, measured per site; the leaf profile (#1923)
ranks them. Owners: attrs-budgets, edit-rollback, pass-list-parity.

## Stage R5 — cost models where LLVM puts them

LLVM: the mid-end does not model register pressure. LICM leaves it to the
back end ([LICM.cpp:17-21](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LICM.cpp#L17-L21));
GVN has one hard-coded PRE exclusion
([GVN.cpp:3646-3656](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/GVN.cpp#L3646-L3656));
unroll and inline compare counts with constant thresholds and stop early
([LoopUnrollPass.cpp:173-205](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L173-L205),
[InlineCost.cpp:1138-1145](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/InlineCost.cpp#L1138-L1145)).
Pressure is modelled after isel and locally: MachineLICM per loop path
([MachineLICM.cpp:1234-1259](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineLICM.cpp#L1234-L1259)),
the scheduler per region
([MachineScheduler.cpp:1476-1505](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineScheduler.cpp#L1476-L1505)),
the allocator per live range from block frequencies on liveness built once
and patched
([LiveIntervals.cpp:159-173](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/LiveIntervals.cpp#L159-L173),
[RegAllocGreedy.cpp:994-1000](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/RegAllocGreedy.cpp#L994-L1000)).

llrm today: gvn prices both numberings with a whole-function spill
forecast ([gvn.rs:355-431](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/gvn.rs#L355-L431),
[profit.rs:559-655](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/profit.rs#L559-L655)),
61% of gvn; hoist clones and prices each candidate run
([hoist.rs:260-420](https://github.com/ali-mosavian/llrm/blob/cc2806006fc4/crates/opt/llrm-transforms/src/hoist.rs#L260-L420)).
Removing gvn's careful numbering alone cost frames +1.5% instructions
(#1883), because the back end does not recover what the mid-end gave up.

Change, in this order: (1) the back end gains what makes mid-end pricing
unnecessary: rematerialization and sinking in the allocator and a
pressure-aware LIR hoist/sink per loop path (MachineLICM's rule), on
liveness built once and patched; (2) then gvn and hoist drop whole-function
pricing for LLVM's local rules; (3) any row that falls below
min(gcc, clang) blocks step 2 for that pass. Gate: rows; counted test:
spill forecasts per pass run = 0 in the mid-end. Ceiling: gvn pricing
~5%, hoist pricing at -Os ~3-4%, plus the allocator's re-solves. Owners:
greedy-port and ssa-spill-m16 (back end), hoist-bounded (gvn/hoist).

## Order and gates

- R1 and R2 together are low single-digit percent of the compile. R3 (rounds)
  and R4 (containers) are the large levers, so the order of work follows that.

- R1 first: R2, R3's per-loop worklist and R5's patched liveness all need
  analyses that survive edits. R4 runs alongside (no output change).
  R3 steps 1-2 can start now; rounds drop after R1. R5 last.
- Every stage reports the gap table (`tools/compile-gap.py`, dist) and
  cycles min-of-7; identical-output stages run identical.py on the corpus
  and bench flags with every `LLRM_CHECK_*` on; output-changing stages are
  judged by rows against min(gcc, clang).
- Each stage ends with its counted budgets in spec §6 as tests, so a later
  change that reintroduces the cost fails CI.
- This plan supersedes nothing in compile-time-spec.md; it orders its
  stages 1, 4, 5 and 7 by LLVM's mechanism and adds R2 and R5.
