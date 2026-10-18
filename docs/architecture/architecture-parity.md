# Architecture parity with LLVM: what carries the gap, and how to prove it early

Pinned sources: LLVM at [`d1106deb71cc`][llvm], llrm main at [`bd6e647f6`][llrm]. The redesign plan is [PR #2040][plan] at `bd88740dc`. Numbers are `d_faces` -O2, `-m32 -march=i486 -fno-inline-functions`, callgrind instructions, unless stated. A number with no repo or [#1994] source is marked `unmeasured`; a session's report that is not yet in the repo is marked "session report".

## 1. TL;DR

`d_faces` -O2 costs 2,903 M instructions in llrm and 415 M in LLVM (6.8x) ([synthesis]). Six architectural differences carry it. They are ranked here by measured cost; the pools overlap.
> **Reference note.** The LLVM allocator figures on this page (greedy 39.5 M, 815 assignments, 78 evictions, per-decision costs) were taken from the distro `llc` 20.1.8, are not reproduced on the pinned or the distro `llc`, and are being re-taken on the pinned 24.0.0git `d1106deb71cc` by regalloc-core-poc. Do not quote them until its old-versus-new lands.


| # | difference | llrm, measured | LLVM | decision |
|---:|---|---|---|---|
| 1 | **Who asks for whole-function facts.** Every frequent pass (`fold`, `decide`, `promote`, `sroa`, `hoist`) asks the fact tower and pays a rebuild per body version | analyses 631 M own over 2,742 computes, 2.8 per pass run; the `Held`/`ThroughMemory`/decide family alone 403 M (12.8%); `r01-fold` 2.4 M own, 27.5 M with its analyses | 0.53 computes per run; `getResultImpl` 5.3 M; InstCombine and SimplifyCFG ask no dataflow; global solves live in a few passes (SCCP, GVN, CVP, IndVars) | **A1, needed** |
| 2 | **Backend data model.** LIR is a persistent `Arc` structure; incrementality comes from diffing old and new bodies; a block added means a full renumber | `regalloc` 415.5 M against greedy's 39.5 M; LIR clean-up 290 M against about 40 M | `MachineInstr` ilist edited in place; `SlotIndexes` renumber locally; `LiveRangeEdit` edits intervals of the edited values only | **A5, needed** |
| 3 | **Convergence by rounds.** The whole pass list repeats up to 4 times | rounds after the first: 35% to 56% of step work; round three 211 M (6.3%); idle runs 361 M with their analyses | a fixed list (InstCombine and SimplifyCFG 8 runs each on `d_draw_faces`), worklists inside the passes, a per-loop queue | **A3, needed after A1** |
| 4 | **Unbounded per-run work.** Whole-function pricing per decision, searches per candidate | `hoist` 366 M own at -O1 (`_movable` 254 M); spill forecast 1.39% (-O2) and 2.52% (-Os); optimizer exponents 1.2 to 1.4 | every query walk capped (MemorySSA 100, MemDep 100, EarlyCSE 500, DSE 150, LSR 200 users); no register pressure in the mid-end | **A4, needed** |
| 5 | **Allocation and copying** | 518 M (18%); about 950 allocations per IR instruction against 19 | `SmallVector`, operands co-allocated, bump arenas for analyses | follows from 1 and 2; slab (#2022) is the constant-factor part |
| 6 | **MIR edit core.** Block order is a `Vec<InstId>`, a use list is a `Vec<Use>` scanned on removal | edit primitives 6.5 M (0.2%) on `d_faces` (session report); 1.5 G of about 5 G on `mem` N=1600 before the erase fixes (session report) | ilist and intrusive `Use` chains: O(1) insert, erase, use removal | **A6, a scaling term, not a `d_faces` term** |

Sources by row: 1 [#1994], [readme], [measured], [plan] 3.1; 2 [plan] 3.3, 3.4, [regalloc]; 3 [plan] 3.2, [spec] 8, [idle]; 4 [spec] 7, [plan] 3.5, [readme]; 5 [allocation], [plan] 1.0; 6 edit-rollback's session report.

Not a cause, with the number: the analysis manager's mechanics (`drop_unpreserved` 2.85 M against `invalidate` 2.63 M; computes on an already-computed body version 1.4%) ([analysis-manager], [#1994]); operands stored inline in `Instruction` (0.2% of instructions) ([plan] 1.0); a scratch arena (ceiling 3% of instructions, likely 0 in cycles) ([spec] 8); caps in `gvn` (100 against 100) ([gvn]); a combine worklist at today's positions and rounds (about 0) ([plan] 3.2); the front end (1.7%).

**The minimum architecture** is six invariants (section 4): facts placed (A1), facts never define identity (A2), convergence inside passes (A3), bounded runs (A4), a backend that edits in place (A5), and an edit core with O(1) primitives and rollback (A6). A1 is the decision the plan does not yet make. The plan's 3.1 updates the tower from the change log for every asker; A1 instead stops the frequent passes from asking. One measured fact points there: on `d_faces` at these flags, `decide` asks 574 branch visits and decides none, and in the whole corpus no memory-derived fact decided a branch ([#1994]).

**Rewrite or incremental.** Incremental. A2, A5 and A6 can land with byte-identical output, each step checked by `identical.py` over 1,583 objects. A1 and A3 change which facts a pass sees and how often it runs, so they change output by nature. Their mechanism lands identical first (solver positions = every position, rounds as today), then the placement changes under the rows rule ([spec] 4.3). A5 is the one rewrite-sized piece, the LIR model under the allocator and the clean-up. It goes strangler-style, one consumer at a time, with identical output.

**Architecture is necessary, not sufficient.** Each invariant's bound in the repo (section 6) takes `d_faces` -O2 to about 1,500 to 2,000 M (3.6x to 4.8x). Parity then needs each pass at a clang-derived per-run budget (plan 4.1, 4.2). Those budgets are reachable only under A1, A3 and A5. Where the tower is cheap (`mem` N=200), `fold` already costs what InstCombine costs: 36.8 against 29.1 M ([gvn]).

**Spikes before the builds** (section 5; S0 is the identity probe already running as plan 0.3):

1. S1, fact placement.
2. S2, cost of a fact-free run.
3. S4, allocator attribution and in-place spill.
4. S6, edit core scaling.
5. S7, caps and pricing.
6. S5, block-local post-RA dataflow.
7. S3, one round with cleanup positions. It needs S1 and S2.

All but S3 start in parallel now. The decision review is at day 10.

## 2. LLVM's architecture

### (a) IR data structures

- A `Use` is a node of an intrusive, doubly linked list hung off the used `Value`; adding and removing it is O(1) ([Use.h#L82-L105][u-use]). `replaceAllUsesWith` walks that list, one pointer swap per use, with no hashing or allocation ([Value.cpp#L518-L551][u-rauw]).
- A `User`'s operands sit in the same allocation as the instruction, so building one costs one `operator new` ([User.cpp#L137-L168][u-user]). A terminator's successors are a range over its operands ([Instruction.h#L1029-L1035][u-succ]).
- A `BasicBlock` holds its instructions in an intrusive `SymbolTableList` (ilist), so insert, erase and move are O(1) with stable pointers ([BasicBlock.h#L64-L76][u-bb]). `comesBefore` reads an order number that the block renumbers lazily when it is stale ([Instruction.cpp#L335-L343][u-order], [BasicBlock.cpp#L636-L645][u-renum]).
- Value handles (`WeakVH`, `AssertingVH`, `CallbackVH`) let a side table follow a value across RAUW and deletion ([ValueHandle.h#L30-L165][u-vh]).

### (b) Pass manager and analysis manager

- After each pass, `PassManager::run` calls `AM.invalidate(IR, PA)`. When everything is preserved it returns at once; otherwise each held result decides whether it survives ([PassManagerImpl.h#L80][u-pmrun], [#L169-L207][u-inval]). Results are computed on the first `getResult` and cached ([PassManagerImpl.h#L131-L166][u-cache]).
- A result's own `invalidate` keeps it across a pass that preserved what it reads. `DominatorTree` survives every CFG-preserving pass ([PassManagerInternal.h#L232-L258][u-resinv], [Dominators.cpp#L71-L78][u-dtinv]).
- Updaters keep results valid instead of recomputing:
  - the dominator tree takes batched edge updates, eagerly or lazily, and recomputes past a size threshold ([GenericDomTreeUpdater.h#L31][u-dtustrat], [GenericDomTreeConstruction.h#L1137-L1184][u-dtupd]);
  - SCEV forgets only the changed loop or value ([ScalarEvolution.cpp#L8714-L8727][u-scevforget]);
  - MemorySSA is kept valid by `MemorySSAUpdater` across insert, remove, move and CFG edits ([MemorySSAUpdater.h#L55-L298][u-mssaupd]).
- Loop passes receive dominators, LoopInfo, SCEV and MemorySSA once per function and must keep them valid ([LoopAnalysisManager.h#L52-L61][u-lam]). A per-loop worklist re-runs only the loop that changed ([LoopPassManager.cpp#L193-L341][u-lpmwl], [LoopPassManager.h#L218-L350][u-lpmupd]).
- The CGSCC pipeline re-runs an SCC only when devirtualization or inlining changed it, at most four times ([CGSCCPassManager.cpp#L367][u-devirt], [#L451-L541][u-cgscc]).
- `LastRunTrackingAnalysis` lets a pass that found nothing skip its next run until any pass changes the IR. InstCombine checks it before asking for any analysis ([LastRunTrackingAnalysis.cpp#L30-L49][u-lrt], [InstructionCombining.cpp#L6258-L6263][u-lrtuse]).

### (c) Pass shapes and their caps

- InstCombine is a worklist over the function's instructions ([InstructionWorklist.h#L25-L31][u-iwl]). A change re-queues users and operands, dead instructions go on the spot, and one outer iteration is the default ([InstructionCombining.cpp#L5780-L5972][u-icrun], [InstCombiner.h#L423-L428][u-icrequeue], [InstCombine.h#L29][u-icmax], [InstructionCombining.cpp#L6199-L6230][u-iciter]). It asks no dataflow analysis. When it changed only instructions, it reports the CFG analyses kept ([InstructionCombining.cpp#L6286-L6291][u-icpa]).
- SimplifyCFG iterates only while it changes ([SimplifyCFGPass.cpp#L229][u-scfg]). SROA works from a worklist of allocas ([SROA.cpp#L188][u-sroa]).
- EarlyCSE is one dominator-tree walk with scoped tables. A load is checked first by comparing two generation counters, and MemorySSA is asked at most 500 times a function ([EarlyCSE.cpp#L76-L79][u-ecsecap], [#L1066-L1103][u-ecsegen]).
- The caps:

  | pass | cap |
  |---|---|
  | GVN | 100 dependences per load ([GVN.cpp#L128-L150][u-gvncap]) |
  | MemoryDependence | 100 instructions per block, 200 blocks ([MemoryDependenceAnalysis.cpp#L74-L82][u-mdcap]) |
  | LICM | MemorySSA 100, promotion 250 ([LICM.cpp#L149-L160][u-licmcap]) |
  | DSE | scan limit 150 and step limits ([DeadStoreElimination.cpp#L126-L162][u-dsecap]) |
  | LSR | gives up above 200 IV users ([LoopStrengthReduce.cpp#L6322-L6336][u-lsrcap]) |

### (d) Pipeline at -O2

`buildFunctionSimplificationPipeline` is one fixed order ([PassBuilderPipelines.cpp#L629-L841][u-fsp]). SROA and EarlyCSE come first ([#L643-L650][u-early]); GVN comes after the loop passes, then DSE ([#L776-L790][u-gvnpos]). InstCombine and SimplifyCFG each have four positions in it ([#L677][u-ic1], [#L768][u-ic2], [#L801][u-ic3], [#L837][u-ic4]). The module simplification pipeline adds more ([#L1242-L1245][u-globalcleanup]), and the inliner runs the function pipeline inside a CGSCC walk ([#L972-L1071][u-inliner]). On `d_draw_faces` that is 77 function-pass runs and 207 loop-pass runs: SimplifyCFG and InstCombine 8 times each, GVN once, DSE once ([measured]). SROA and EarlyCSE cut the body from 2,663 to 1,146 instructions before the first InstCombine. InstCombine is skipped 32 of 88 times by last-run tracking.

The expensive global work runs at a few fixed points: IPSCCP and SCCP, one GVN, CVP and LVI on demand, IndVars per loop. The cheap local cleanup runs at many points. Measured analysis builds over the whole file are 55 DominatorTree, 37 LoopInfo, 35 SCEV, 27 MemorySSA, 44 AAManager and 1 BlockFrequency ([measured]).

### (e) MemorySSA and alias analysis

- MemorySSA keeps an access list and a defs list per block as ilists ([MemorySSA.h#L753-L755][u-mssalist]). `OptimizeUses` gives every use its clobbering access on the first walker use, not at build (`ensureOptimizedUses`), and stores it on the access ([MemorySSA.cpp#L1307-L1341][u-optuses], [#L2203-L2212][u-ensure]). A later query is a field read. Otherwise the walk is capped at `memssa-check-limit` 100 and the answer is stored back ([MemorySSA.cpp#L76-L78][u-msscap], [#L2556-L2593][u-msswalk], [MemorySSA.h#L329-L356][u-mssaopt]).
- BasicAA keeps no state. Each query walks to the underlying objects (at most 10 steps) and recurses to depth 512 at most. It caches the pair in `AAQueryInfo`, which lives for one query, or for a batch under `BatchAAResults` ([BasicAliasAnalysis.cpp#L1684-L1720][u-baa], [ValueTracking.h#L52][u-uodepth], [AliasAnalysis.h#L608-L622][u-batchaa]).
- `computeKnownBits` and `computeKnownFPClass` are per value with depth 6 ([analysis-avoidance]).

### (f) Backend

- The DAG combiner is a worklist seeded with every node, run four times per block's DAG inside selection, with known bits and use counts at hand. Combine is 20.1 M of selection's 51.3 M on `d_faces` ([DAGCombiner.cpp#L1845-L1962][u-dagrun], [SelectionDAGISel.cpp#L977-L983][u-dagcomb], [lir-cleanup]).
- `MachineBasicBlock` holds an ilist of `MachineInstr` ([MachineBasicBlock.h#L138-L152][u-mbb]). Each register operand links into a def/use list whose head `MachineRegisterInfo` keeps per virtual register ([MachineOperand.h#L190-L192][u-mop], [MachineRegisterInfo.h#L87-L89][u-mri]).
- `SlotIndexes` is an ilist of index entries 16 apart. A new instruction takes the midpoint of its neighbours, and only a local range renumbers when the gap is gone ([SlotIndexes.h#L528-L566][u-slot], [SlotIndexes.cpp#L193-L218][u-renumidx]).
- `LiveIntervals` is computed once and edited: `shrinkToUses`, `handleMove`, `repairIntervalsInRange` ([LiveIntervals.h#L205][u-lis], [#L356][u-lis2], [#L378][u-lis3]). A split or spill recomputes only its own values ([LiveRangeEdit.h#L42-L222][u-lre], [SplitKit.h#L263][u-splitkit]).
- Greedy allocation takes ranges from a priority queue by size ([RegAllocGreedy.cpp#L420-L437][u-rgq], [RegAllocBase.cpp#L88-L155][u-rabase]). Splitting is a ladder tried only after assignment and eviction fail ([RegAllocGreedy.cpp#L1975-L2010][u-trysplit]).
- Interference comes from a `LiveIntervalUnion` per register unit, an interval map with a tag, plus a 32-entry `InterferenceCache` per (register, block) ([LiveIntervalUnion.h#L42-L62][u-liu], [InterferenceCache.h#L137][u-icache]). Edge bundles are computed once ([RegAllocGreedy.cpp#L240][u-bundles]).
- A machine pass declares what it keeps. Greedy preserves the CFG, `LiveIntervals`, `SlotIndexes`, `LiveStacks`, `VirtRegMap` and `LiveRegMatrix` ([RegAllocGreedy.cpp#L341-L364][u-rgpres]).
- The x87 stackifier reads the live-in lists the allocator left on each block; it solves no liveness of its own ([X86FloatingPoint.cpp#L107-L120][u-fpstack]).

### (g) Memory

- `SmallVector` keeps a few elements inline ([SmallVector.h#L1166][u-sv], [#L1251][u-sv2]).
- `DenseMap` is open addressing with linear probing and tombstone-free deletion, flat in one allocation ([DenseMap.h#L12-L15][u-dm], [#L799-L829][u-dmprobe]).
- `BumpPtrAllocatorImpl` hands out 4 KB slabs and frees them all at once ([Allocator.h#L65-L70][u-bump]). It backs SCEV ([ScalarEvolution.h#L2595][u-scevbump]) and LiveIntervals ([LiveIntervals.h#L67-L68][u-lisbump]).
- `MachineFunction` takes its instructions and operands from recyclers over its own bump allocator ([MachineFunction.h#L327-L341][u-mf], [Recycler.h#L37-L112][u-recycler]). SelectionDAG nodes come from a `RecyclingAllocator` ([SelectionDAG.h#L272-L289][u-sdalloc]).
- On `d_faces` that is 62,881 allocations, 38 MB, with a 5.5 MB live peak ([allocation]).

## 3. llrm today, layer by layer

| layer | llrm structure | differs from LLVM by | measured cost |
|---|---|---|---|
| (a) ids and arenas | `ValueId`, `BlockId` and `InstId` are dense u32 indices into per-function `Vec` arenas. They are never reused; erase sets a tombstone ([module.rs#L1-L46][m-ids], [#L284-L328][m-func], [edit.rs#L176-L199][m-create]) | same guarantees as stable pointers, with dense side tables | none: a strength |
| (a) instruction | `Instruction { operands: Vec<Operand>, metadata: Vec<..> }`, a heap block per operand list ([module.rs#L70-L94][m-inst]) | operands are not co-allocated | inline operands: at most 2.7% of allocations and 0.2% of instructions ([plan] 1.0) |
| (a) block order | `Block.instructions: Vec<InstId>`. Insert and move scan for the position, then shift ([edit.rs#L201-L218][m-slot], [#L316-L385][m-insert]) | O(block) against ilist O(1); "comes before" is a scan | with use removal: 6.5 M (0.2%) on `d_faces`; 1.5 G of about 5 G on `mem` N=1600 before the erase fixes (session report) |
| (a) use lists | `value_uses`/`block_uses: Vec<Vec<Use>>`, kept by the editor ([module.rs#L525-L538][m-users], [edit.rs#L47-L78][m-adduse]). `remove_use` is a `retain`. `replace_all_uses_with` is O(U²); `replace_value` is O(U) ([edit.rs#L489-L520][m-rauw]) | LLVM's per-use removal is O(1) | in the 0.2% above |
| (a) change log | `Change` per instruction and block, `Mark` positions, `Touched` digest, `Reach` through the user lists, `Epochs` ([module.rs#L133-L282][m-log], [touched.rs#L31-L74][m-touched], [#L154-L189][m-reach]) | finer than `PreservedAnalyses`; no inverse yet | — |
| (a) speculation | `clone()` and `fork()`/`adopt()`. Rollback is restoring a copy (lsr, hoist, sink and about 15 more sites) ([module.rs#L675-L699][m-fork], [lsr.rs#L603-L617][m-lsrclone], [hoist.rs#L320-L325][m-hoistclone]) | no checkpoint and undo | 144 clones at -O2, about 0.1% of the compile ([plan] 2.7) |
| (a) interning | types and constants hash-consed in `Context` ([context.rs#L45-L92][m-ctx], [types.rs#L88-L108][m-types]). Memory objects are interned by their facts (addressed, captured) ([plan] 0.3) | an analysis result is part of an object's identity | skipping one `points_to` solve changes 131 of 1,583 objects ([plan] 0.3); blocks every skip |
| (b) analysis manager | `Analyses` per function. Entries are keyed by `Mark` plus an attribute stamp plus the `Outer` pointer. Flags `INCREMENTAL`, `SKIPS`, `SOUND`. `PreservedAnalyses` with `unless_unchanged`; `drop_unpreserved` moves incremental entries to `kept` ([passes.rs#L925-L1025][m-entry], [#L1295-L1468][m-get], [#L1526-L1563][m-drop]) | the mechanism is LLVM's, with a finer record | invalidation 2.85 M against 2.63 M ([analysis-manager]); same-version computes 41.3 M (1.4%) ([#1994]) |
| (b) what is updated | Dominators, Registers, TableLoads, Counted, DominatedEdges, Bounded, PlainAccesses and memory SSA `Accesses` are incremental. PointerValues, Pointers, CallEffects, Writes and ExposedFrames are skips. `Annotated` has neither and is dropped on any change ([manager.rs#L1088-L1177][m-annot], [#L1343-L1452][m-regs]) | updaters exist; most edits reach what a fact reads (94% of `bounded` and `exposed-frames` reruns) | analyses 631 M own; the tower family 403 M ([#1994], [plan] 3.1) |
| (b) who asks | `fold` is the first asker of almost every tower piece. `Held::of` is asked by `decide`, `fold`, `promote` and `floatfold` ([#1994]) | LLVM's frequent passes ask none of these | `fold` 2.4 M own, 27.5 M with analyses; `decide` 0.47 M against 13.0 M ([readme]); `decide` decides 0 of 574 visits on `d_faces` |
| (c) pass trait | `FunctionPass::run(unit, analyses) -> PreservedAnalyses`. "Changed" means not all preserved ([passes.rs#L2105-L2132][m-trait]) | same contract | — |
| (c) pass shapes | `fold`: one sweep over `consts` facts, invalidating mid-pass ([fold.rs#L177-L195][m-fold]). `algebraic`: whole-function sweeps to a local fixed point, users not re-queued ([algebraic.rs#L231-L258][m-alg]). `phiopt` restarts from the first block after each rewrite ([phiopt.rs#L138-L162][m-phiopt]). `gvn` sweeps in dominator order on two forks, kept by price ([gvn.rs#L366-L435][m-gvn]). `hoist` rescans per loop to a fixed point ([hoist.rs#L671-L743][m-hoistloop]) | sweeps and rounds instead of worklists | a combine worklist at today's positions and rounds saves about 0 ([plan] 3.2): the rewriting is not the cost |
| (c) caps | `MAX_NEAREST_WRITES = 100` on one of three walker entries; `clobbers` uncapped. No caps in `fold`, `algebraic`, `phiopt`, `dead`, `decide`, `gvn`, `hoist` ([memoryssa.rs#L1019-L1025][m-cap]) | LLVM caps every query walk | slopes: `dse` 3.9x, `through-memory` 3.7x, `fold` 3.6x per doubling before #2013/#2015 ([gvn]) |
| (d) pipeline | `every` list of 25 passes in `Fixed`, `ROUNDS = 4` at -O2, a fixed-point limit `max(16, size+1)`, a skip for settled passes ([pipeline.rs#L253][m-rounds], [#L483-L532][m-every], [#L993-L1069][m-fixed]). `PassManager::over` is pass-major across functions ([passes.rs#L2360-L2498][m-over]) | rounds instead of positions | rounds after the first: 35% to 56% of step work; global `rounds=1` lost 25 to 51 rows at -O1 ([plan] 3.2) |
| (e) memory SSA | `Accesses` is incremental only for moves and inert edits. The `Structure` is cached by `Mark`; each client builds a view with walk memos that die with it ([memoryssa.rs#L237-L427][m-mssa], [#L1666-L1689][m-mssaview]) | no clobber stored on the access | walker 133 M over 4,787 queries (27,750 each) against 286 queries (about 11,000 each); structure 71 M ([memoryssa]) |
| (e) alias | whole-function solves cached as entries: `point_values`, `points_to`, `annotated` ([alias.rs#L2746][m-pv], [#L3612-L3640][m-annotated]). Escape-before is per query ([alias.rs#L215][m-escbefore]) | pays per access (`MemRef::of`), not per query | `MemRef::of` 109 M over 185,644 calls; 104,236 overlap queries against 10,624 ([alias]) |
| (f) LIR | `Insn` with `Vec` fields and `OnceLock` caches; `Insns = Arc<[Arc<Insn>]>`, edited by copying the slice; blocks copy-on-write ([lir.rs#L106-L157][l-insn], [#L629-L677][l-block], [#L929-L995][l-body]) | immutable bodies against an in-place ilist | backend peak live heap is LIR copies ([allocation]) |
| (f) slots and uses | `Indexes` numbers the whole function, then `patch` diffs old against new body by `Arc` identity. A new block means a full renumber. `Postings` is a thread-local index re-synced per query by comparing every block ([intervals.rs#L97-L143][l-idx], [#L209-L371][l-patch], [postings.rs#L600-L680][l-post]) | O(blocks) per edit against O(gap) | `Facts` rescans of untouched values about 50 M ([plan] 3.3) |
| (f) intervals | `Interval { segments: Vec<Segment> }`, built whole. Updated by diffing against the last answer, falling back to a whole walk past a quarter of instructions or a third of values ([intervals.rs#L1040-L1214][l-ivupd]). `Facts::of` recomputes everything on a new numbering epoch ([allocate.rs#L1576-L1801][l-facts]) | no `LiveRangeEdit` | interval recompute avoidable at most 11 M (0.4%) ([plan] 2.5) |
| (f) allocator | greedy queue with stages, `LiveUnion` holder lists, no interference cache. A split clones the block list and rebuilds touched blocks ([allocate.rs#L1993-L2585][l-queue], [liveunion.rs#L183-L303][l-union], [splitkit.rs#L586-L814][l-carve]) | edits by copy, interference without a cache | 415.5 M against 39.5 M; split path 132 M, of it carving O(body) 29 M ([plan] 3.3); the rest `unmeasured` by event |
| (f) clean-up | `Peephole::once` is 44 whole-body steps with a per-block `needs` gate. `jumps` repeats to equality ([peephole.rs#L301-L383][l-peep], [jumps.rs#L186-L212][l-jumps]) | LLVM rewrites at selection and keeps post-RA passes block-local | clean-up 290 M; 96.5% of block visits idle; peephole 97.1 M against 24.3 M ([plan] 3.4); per-block idle skip worth 10.4 M, the three dataflow passes about 40 M (session report) |
| (g) memory | slab allocator in flight (#2022), dense `IdSet`/`IdMap`, `BlockList` inline ([dense.rs#L1-L52][m-dense], [blocklist.rs#L19-L25][m-bl]) | no arenas per analysis | 518 M (18%); slab -4.2% instructions, -2.9% cycles ([plan] 1.1) |

## 4. The target architecture

Each decision is an invariant with a counter that can fail a test. Every pass still takes MIR and returns MIR, and names nothing about the machine. Each fact keeps one owning analysis.

**A1. Facts are placed.**
- A pass that runs more than three times per function asks only local facts or per-query facts with a cap: known bits to a depth, one access's `MemRef`, escape-before. It never triggers a whole-function solve.
- Whole-function solves run at named solver positions in the pass list, as SCCP, GVN, CVP and IndVars do in LLVM: constants through memory, float facts, loop bounds, provenance. Each solve either rewrites at once or publishes a conservative fact on the value, gcc's way: a new value starts as "unknown".
- Between solver positions a frequent pass reads the held result while it is current. Otherwise it reads "unknown", never a rebuild.
- Why placement and not per query: where consumers read most of a solve, a per-query form saves nothing. `annotated` asks 87% of what `point_values` solves ([plan] 2.1), and a combine worklist that still asks the tower saves about 0 ([plan] 3.2). What LLVM does differently is not ask from the frequent passes at all.
- Counters: `analysis_computations / pass_runs <= 0.5`; whole-function solves per function at most the number of solver positions.

**A2. Facts never define identity.** An interned object or cell has a context-free identity. Whether a solve ran, and what it said, cannot change any id, spelling or iteration order. Oracle: `LLRM_CHECK_INTERN=SEED`, plus the skip prototype at 0 differing objects ([plan] 0.3). A1 cannot be judged by identical output until A2 holds.

**A3. Convergence is inside passes.**
- Cleanup passes are worklists. They are seeded once in instruction order, re-queue users and operands through the use lists on every change, and delete dead code on the spot.
- They run at several fixed positions, LLVM's sandwich, instead of the whole list repeating.
- Loop passes take loops from a per-loop worklist.
- Rounds are 1 at -O1 to -Os; -Omax keeps the fixed point.
- Counter: idle runs per compile. `LLRM_CHECK_FIXPOINT` runs every pass once more after convergence and fails on any change.

**A4. A run's work is bounded.**
- Every query walk has a named cap, LLVM-sized, set by a corpus sweep.
- No mid-end decision prices the whole function; pressure is the backend's (R5).
- A full run costs at most 300 instructions per IR instruction ([spec] 1.2). No ladder exponent is above gcc's + 0.05.
- Counter: `visited` per pass run at most 2x the function size.

**A5. The backend edits in place.**
- LIR lives in an arena with stable ids and MIR-shaped operand arrays.
- Each virtual register has a def/use list kept by the editor.
- Slot numbers sit on the instructions, with gaps, and renumber locally.
- Intervals are built once per function; a spill or split recomputes only the values it touched. Interference is a union per register with a version, plus a cache per (register, block).
- Post-RA dataflow is block-local; rewrites that need no physical register move to selection.
- Counters: `full_interval_builds = 1`; allocator `visited` at most 40x the edited range plus 4x the touched blocks ([spec] 6).

**A6. The edit core is O(edit), with rollback.**
- Insert, erase, move and use removal are O(1) amortised: per-block gapped order numbers, and a back-index from each operand into its use list.
- RAUW is O(uses).
- Speculation is checkpoint and rollback on the live function. There are no clones in the pipeline.
- Counters: `function_clones = 0`; no whole walk inside the editor.

A6 is not a `d_faces` lever (0.2%). It is needed for slopes (the `mem` ladder) and as the base of A3's worklists and A4's pricing-free speculation.

What A1 to A6 deliberately leave out: an LLVM-shaped analysis manager (llrm's is equivalent and finer); intrusive pointers in Rust (dense ids with back-indices give the same O(1) and stay safe); a per-pass bump arena (the slab is the constant factor; the count falls with A1 and A5).

## 5. Early verification spikes

Rules for every spike:
- a throwaway branch, never merged;
- the base binary built from the branch point;
- callgrind instructions on `d_faces` -O1/-O2/-Os, plus the ladder shapes named;
- cycles only on a quiet host;
- `identical.py` over 1,583 objects, or the rows rule where output moves;
- the kill criterion is fixed before the build;
- tests stay under 10% of wall clock;
- the result goes on [#1994] and, if killed, into [spec] 8.

| order | spike | invariant | owner | effort (session-days) | starts |
|---:|---|---|---|---:|---|
| 0 | S0 identity (plan 0.3, running) | A2 | objectref-identity, keep-analyses | running | now |
| 1 | S1 fact placement | A1 | attrs-budgets (step 0), edit-rollback with keep-analyses (step 1) | 1 + 4 | now |
| 2 | S2 a fact-free run against InstCombine | A1, A4 | pass-list-parity, with llvm-compare for the LLVM column | 2 | now |
| 3 | S4 allocator attribution, then an in-place spill | A5 | greedy-port (allocator), ssa-spill-m16 (LIR model) | 1 + 5 | now |
| 4 | S6 edit core on the ladders | A6 | edit-rollback | 2 | now |
| 5 | S7 caps and pricing | A4 | hoist-bounded (gvn, hoist), greedy-port (lsr) | 2 | now |
| 6 | S5 block-local post-RA dataflow | A5 | ssa-spill-m16 | 3 | now |
| 7 | S3 one round with cleanup positions | A3 | pass-list-parity, hoist-bounded | 5 | after S1 step 1 and S2 |

### S1. Fact placement (A1)

- **Question.** Do the tower's facts need to be fresh at every position of every round, or only at a few solver positions?
- **Step 0 (counter, 1 day).** Count, per (pass, round, family), the rewrites and decisions that read a tower fact: `ThroughMemory`, `FloatFacts`, `Bounded`, `Annotated`-based alias answers, `Held`. Run `d_faces`, the 71 vsgcc programs and QCport at -O2 and -Os. The output is the share of fact-enabled rewrites made at each position.
- **Step 1 (branch, 4 days).** Add a switch: tower families are computed only at the positions step 0 names (for example the first `fold` and `decide` of round 1 and the `fold` after `gvn`). Elsewhere a pass gets the held result if it is current, else "unknown". Then measure:
  - computes per run;
  - the tower's own instructions;
  - `d_faces` instructions at three levels;
  - objects that differ, split by `LLRM_CHECK_INTERN` into identity and real causes;
  - vsgcc rows and bench cells.
- **Kill, if any holds:**
  - step 0 shows under 90% of fact-enabled rewrites at three or fewer positions per function;
  - step 1 saves under 10% of `d_faces` -O2;
  - computes per run stay above 1.0;
  - a vsgcc row crosses above min(gcc, clang) or a bench cell gets worse, and one more solver position does not recover it.
- **Tells the others.**
  - Pass: 2.2's updater and 3.1's tower updaters are not needed. 2.3's MemorySSA builds fall with the solves. Rounds get cheap (S3), because an idle run costs its own work only.
  - Fail: facts are needed everywhere, and 3.1 as the plan has it (updaters from the change log) is the path. Parity then needs each tower fact at LLVM's per-query cost, `ThroughMemory` as a capped per-load walk.

### S2. A fact-free run against InstCombine (A1, A4)

- **Question.** Once the tower is not asked, does a run cost what LLVM's costs?
- **Build (2 days).** Start from `notkept/combine-worklist` ([plan] 3.2): fold's integer rules, `algebraic`, `phiopt`, `dead` and decide's branch fold as one worklist over the use lists, with local facts only.
- **Measure.** Per-run instructions per IR instruction against InstCombine's per run on `mem`, `arith` and `mixed` at N=200 to 1600 (callgrind; llvm-compare supplies the LLVM column). Also the exponent from 400 to 1600, and the share of edit primitives inside a run.
- **Kill, if any holds:**
  - above 2x InstCombine per run per IR instruction at N=200;
  - the exponent is above 1.05.

  Then the cost is in the rules or the IR walk, not the facts. S6 rises to first, and phase 4's per-pass budgets come before S3.
- **Tells the others.** It gives S3 its cleanup instance and its cost per position. An edit-primitive share above 20% of a run makes A6 a `d_faces` term after all.

### S3. One round with cleanup positions (A3)

- **Question.** Can LLVM's placements plus a cheap combine replace rounds 2 to 4 without losing rows?
- **Build (5 days, on S1 and S2).** Rounds = 1 at -O2. Place S2's combine after each pass the per-pass ablation names as an enabler (plan 3.2 (b): run P in round 1 only, list the objects that change, name the enabler). Add a per-loop queue for `hoist` and `loopmotion` only.
- **Measure.** vsgcc rows at four levels, bench cells, `d_faces` instructions and idle runs.
- **Kill, if any holds:**
  - a row crosses above min(gcc, clang) whose enabler is a global solve, not a local rewrite;
  - the saving is under 8% at -O2.
- **If it fails.** Keep the rounds. With S1 passed they cost their own work only; the 361 M of idle runs with analyses falls toward their own share. Re-measure before any further structural pipeline work.

### S4. Allocator attribution, then an in-place spill (A5)

- **Question.** Is the 415.5 M the representation (whole-body re-derivation per edit) or the decisions (how many attempts)?
- **Step 0 (1 day).** Attribute `regalloc` by event: queue pops, assignment tries, evictions, splits, spills, `Facts::of`, `Indexes::patch`, `Postings` re-syncs, full renumbers. Count `full_interval_builds`, allocator `visited` per edit and body copies per carve (spec 6). Give the cost per event against LLVM's: greedy 39.5 M in all; 28 k per `InlineSpiller::spill`.
- **Step 1 (5 days).** One spill path on an in-place `Vec<Insn>` arena body: slots on instructions, per-value use lists, and the spilled value's interval recomputed from its own uses and defs. No `patch`, no re-sync. Measure per-spill instructions and `regalloc` on `d_faces`, diamond 800 and the `live` and `cells` axes, with identical allocation (`LLRM_CHECK_INTERVALS` against the whole build).
- **Kill, if any holds:**
  - step 0 finds whole-body work per edit below 1% of the compile (the plan's rule);
  - step 1 cuts the cost per spill by less than 3x.

  Then the cost is the decisions: price them as the in-run decisions of [spec] 4.2, and leave the LIR model.
- **Tells the others.** S5's dataflow needs the same use lists. A pass here commits ssa-spill-m16 to the LIR arena for the clean-up too.

### S5. Block-local post-RA dataflow (A5)

- **Build (3 days).** Make `copyprop`, `spillforward` and `postrasink` block-local, as LLVM's `MachineCopyPropagation` is, on a throwaway branch.
- **Measure.** Peephole instructions against 97.1 M, and the rows. Block-local loses cross-block forwards by design.
- **Kill, if any holds:**
  - under 15 M saved;
  - a row is worse that the allocator's own edits (S4) do not recover.
- **If it fails.** The clean-up gap is per useful visit, and phase 4's budgets come first.

### S6. Edit core on the ladders (A6)

- **Measure (2 days).** After the erase fixes, take the inclusive share of `insert`, `detach`, `move_to`, `remove_use`, `replace_all_uses_with` and the position scans on every ladder shape at N=1600, with its exponent. Prototype a back-index for use removal and gapped per-block order numbers under `dead` and `fold` only.
- **Kill (do not build A6 beyond rollback), if all hold:**
  - edit primitives are under 3% at N=1600 on every shape;
  - they add no exponent.
- **Tells the others.** S2's edit share decides the same question on real runs.

### S7. Caps and pricing (A4)

- **Measure (2 days).**
  - Reconcile pricing: inclusive `View::forecast`, `Pressure::of`, `profit::weighted`, `lsr` `Problem::total_of` and `spill::Run::fit` at -O2 and -Os on `d_faces` and `loop` N=800 (plan 3.5 step 0).
  - Count, over the corpus, the uncapped walks (`clobbers`, `clobbers_ignoring_invariance`, `hoist`'s `_movable`) that pass 100 steps.
- **Kill, if all hold:**
  - pricing is under 1% at both levels;
  - no walk passes 100 steps in the corpus.

  Then caps are at parity, and A4 is per-pass algorithm work (plan 4.1).
- **Tells the others.** Pricing above 2% puts R5 (3.5) before 4.1. It needs S4's backend to recover what the mid-end gives up ([#1883]).

### What the spikes decide together

| S1 | S3 | consequence |
|---|---|---|
| pass | pass | A1 + A3 is the optimizer's architecture. Stop 2.2, 2.3 and the 3.1 updaters except Dominators, Loops and Registers |
| pass | fail | A1 alone; rounds stay, cheap |
| fail | — | 3.1 updaters as planned; each tower fact needs an LLVM-priced per-query form |

S4 and S5 decide the backend independently.

## 6. The plan against the architecture

What each invariant can take off `d_faces` -O2 (2,903 M), bounded by measured pools. The pools overlap, so the sum is optimistic.

| invariant | pool (measured) | bound | basis |
|---|---|---:|---|
| A1 | analyses 631 M own; tower family 403 M | -320 to -520 M | the family at a fifth, up to the whole pool at 0.5 computes per run against 2.8: `unmeasured` projection |
| A3 | rounds after the first, 35% to 56% of step work; round three 211 M | -200 to -290 M | [plan] 3.2's target of -8% to -10% |
| A5 | `regalloc` 415.5 M; clean-up 290 M | -355 to -525 M | `regalloc` to the 100 M budget; clean-up to 250 M (realistic) or 80 M (budget) ([spec] 1.3) |
| A4 | forecast 1.39%; per-pass own work | -40 M, then phase 4 | [plan] 3.5; 4.1 |
| A6 | edit primitives 6.5 M | about 0 on `d_faces` | session report |
| all | | about 1,500 to 2,000 M left (3.6x to 4.8x) | the rest is per-run cost, phase 4 at clang's budgets |

States are from the plan's status table at `bd88740dc`.

| step | state | relation to A1 to A6 | recommendation |
|---|---|---|---|
| 0.1 baseline | in progress | prerequisite | keep; add computes per (pass, family) for S1 |
| 0.2 counters | draft #1948 | prerequisite: every invariant's counter | keep; add `whole_solves_per_function` and `idle_runs` |
| 0.3 `ObjectRef` identity | testing | **on path, A2**; blocks A1's identical judgement | keep, first |
| 1.0 inline storage | to do | constant factor | keep the probe; build only sites above 0.5% |
| 1.1 slab | ready | constant factor that pays (cycles -2.9%) | land |
| 1.2 dense ids | merged, ready | constant factor | land |
| 1.3 escape phase | ready | constant factor inside the tower | land; nothing further on escapes |
| 1.4 backend copies | ready | constant factor; toward A5 | land |
| 1.5 mask at each branch | merged | parity of selection (CodeGenPrepare) | done |
| 1.6 gvn, walker, lsr slopes | ready | **on path, A4** | land |
| 1.7 dse slope | partly merged | **on path, A4** | land |
| 1.8 deep nests | ready | A1-shaped (lazy scopes are per query) | land |
| 1.9 exact ThroughMemory | merged | correctness | done |
| 1.10 held PRs | open, drafts | #1958 is a detour that A5 replaces; #1962, #1965 constant | land #1958 only as it stands; close #1962 and #1965 unless at least 0.5% on main |
| 2.1 point_values | stopped | — | stays stopped |
| 2.2 Annotated per access | to do | detour if S1 passes | **hold until S1** |
| 2.3 persistent MemorySSA | to do | partly on path (LLVM keeps the clobber on the access); its builds fall with A1 | **hold until S1**; then size what is left |
| 2.4 split path A' | A' only | detour; A5 removes the carve copy | **stop**; fold A' into S4 step 1 |
| 2.5 interval recompute | closed | — | — |
| 2.6 `Frequencies` dense | to do | constant factor | keep, low |
| 2.7 rollback | draft #1945 | **prerequisite, A6** | land |
| 2.8 scaling shapes | to do | **on path, A4** | keep as measurement |
| 2.9 Vec growth | to do | constant factor | keep, low |
| 3.1 facts from the change log | design | on path only if S1 fails; otherwise only for the cheap many (Registers, Dominators, Loops) | **reshape**: S1 first; the design page names solver positions and what stays per query |
| 3.2 one pipeline pass | design | **on path, A3**; the combine alone is about 0 | run S2 and S3 now; the design follows S3 |
| 3.3 allocator on the LIR core | design | **on path, A5** | S4 now |
| 3.4 LIR clean-up | per-visit probe | **on path, A5** | S5 now; per-block idle skip (10.4 M) is not worth a PR |
| 3.5 pricing | after 3.3 | **on path, A4** | S7 now; build after S4 |
| 4.1 per-pass budgets | measure first | **on path, A4** | turn into counted tests as each invariant lands |
| 4.2 budgets from clang | to do | prerequisite: the target S2 is judged against | do now (1 day, llvm-compare) |
| 4.3 front end | parked | not on path (1.7%) | stays parked |
| B1 to B3 | — | measurement | keep, but they no longer gate phase 3 |

**New steps:**
- S1 (fact placement), with a design page "solver positions" once it passes;
- S6 (edit core on the ladders);
- a counter for whole solves per function.

**Reordering:**
- S0 to S7 run now, beside phase 1, as the plan's head already allows for phase 3.
- No phase-2 build starts before S1 reports, except 2.7.

**Stop doing:**
- updaters for tower families whose askers A1 would remove (2.2, the tower part of 3.1) until S1;
- split-path and interval patches on the `Arc` LIR (2.4 A', any successor of 2.5);
- per-block idle skips in the peephole;
- optimizer micro-PRs below 0.5% that touch the tower, once phase 1 lands;
- idle-run skips of any kind (row 5 is closed);
- further early-CSE or pass-reorder variants until S3.

## 7. Migration

The strangler pattern for each invariant: build the new structure beside the old, have both answer under a check mode, move consumers one at a time, then delete the old. Each stage's oracle is named.

| stage | what lands | output | oracle |
|---|---|---|---|
| M0 | counters of 0.2 and the new ones; check modes on in `identical.py` | identical | counter tests in CI |
| M1 (A6) | checkpoint and rollback (2.7); then back-indexed use removal and gapped order numbers if S6 says | identical | `LLRM_CHECK_ROLLBACK` (rollback equals the clone); `check_uses` |
| M2 (A2) | context-free object identity | identical | `LLRM_CHECK_INTERN=SEED` at several seeds; the skip prototype at 0 differ |
| M3a (A1, mechanism) | solver positions in the manager, set to every position (today's behaviour) | identical | `identical.py`; `LLRM_CHECK_ORDER` |
| M3b (A1, placement) | positions cut to S1's list, one family per PR | rows rule | `LLRM_CHECK_PLACED`: at each non-solver ask, compute fresh and assert the held or "unknown" answer is no stronger than the fresh one (soundness); rows against min(gcc, clang) |
| M4a (A3, mechanism) | combine worklist at today's positions and rounds | identical | `identical.py` |
| M4b (A3, rounds) | placements, then rounds to 1 per level | rows rule | `LLRM_CHECK_FIXPOINT`; rows; bench cells |
| M5 (A5) | LIR arena beside `LirBody`, converted at the allocator's entry and exit. Then consumers move one at a time: spill, split, evict, `Facts`, then the peephole steps. `Arc<Insn>` bodies are deleted last | identical | `LLRM_CHECK_INTERVALS` (edited against whole build); `identical.py` per consumer |
| M6 (A4) | caps by corpus sweep; pricing moves to the backend after M5 | caps identical where the sweep shows no query reaches them; pricing by rows | `LLRM_CHECK_CAP`; rows |

**Risks and their guards:**

- **Order dependence.** A worklist visits instructions in a different order than a sweep, and two equivalent folds can leave different text. Seed the worklist in instruction order and pop it in that order, as InstCombine's `prepareWorklist` does. M4a must be byte-identical before M4b starts. A difference at M4a is a dependence on order and gets its own fail-first test.
- **The interner.** Fact-spelled objects are why a skipped solve moves 131 objects. Without M2, every A1 skip reads as an output change. Run M2 before M3b and judge M3b's identity-only differences by `LLRM_CHECK_INTERN`.
- **Stale but sound facts.** A held fact at a non-solver position must be weaker than or equal to the fresh one, never different in kind. A result that depends on when the solve ran is the #1909 class. `LLRM_CHECK_PLACED` and `LLRM_CHECK_ORDER` guard it.
- **Determinism.** New per-compile tables use dense ids or seeded maps (the existing ratchets). No decision may iterate a hash map keyed by an interned id.
- **Two structures at once.** During M5 the backend holds both `LirBody` and the arena. Peak heap is checked per stage by DHAT against the 20 MB after #2021 and #2027. The conversion cost is counted and must fall to 0 when the last consumer moves.

[llvm]: https://github.com/llvm/llvm-project/tree/d1106deb71cc81016406a1917d0508d2df296266
[llrm]: https://github.com/ali-mosavian/llrm/tree/bd6e647f643dc71ff11a0ad7af8f00a008be2a82
[plan]: https://github.com/ali-mosavian/llrm/blob/bd88740dcc2d4593aca789692861cffcf56f473f/docs/architecture/redesign-plan.md
[#1994]: https://github.com/ali-mosavian/llrm/issues/1994
[#1883]: https://github.com/ali-mosavian/llrm/pull/1883
[synthesis]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/synthesis.md
[measured]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/measured.md
[readme]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/readme.md
[analysis-manager]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/analysis-manager.md
[memoryssa]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/memoryssa.md
[alias]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/alias.md
[gvn]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/gvn.md
[allocation]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/allocation.md
[lir-cleanup]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/lir-cleanup.md
[spec]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/architecture/compile-time-spec.md
[analysis-avoidance]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/architecture/analysis-avoidance.md
[u-use]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Use.h#L82-L105
[u-rauw]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Value.cpp#L518-L551
[u-user]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/User.cpp#L137-L168
[u-succ]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Instruction.h#L1029-L1035
[u-bb]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/BasicBlock.h#L64-L76
[u-order]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instruction.cpp#L335-L343
[u-renum]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L636-L645
[u-vh]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/ValueHandle.h#L30-L165
[u-pmrun]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerImpl.h#L80
[u-inval]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerImpl.h#L169-L207
[u-cache]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerImpl.h#L131-L166
[u-resinv]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerInternal.h#L232-L258
[u-dtinv]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L71-L78
[u-dtustrat]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/GenericDomTreeUpdater.h#L31
[u-dtupd]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1137-L1184
[u-scevforget]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8714-L8727
[u-mssaupd]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSAUpdater.h#L55-L298
[u-lam]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/LoopAnalysisManager.h#L52-L61
[u-lpmwl]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopPassManager.cpp#L193-L341
[u-lpmupd]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Scalar/LoopPassManager.h#L218-L350
[u-devirt]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CGSCCPassManager.cpp#L367
[u-cgscc]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CGSCCPassManager.cpp#L451-L541
[u-lrt]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LastRunTrackingAnalysis.cpp#L30-L49
[u-lrtuse]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L6258-L6263
[u-iwl]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/InstructionWorklist.h#L25-L31
[u-icrun]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5780-L5972
[u-icrequeue]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/InstCombine/InstCombiner.h#L423-L428
[u-icmax]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/InstCombine/InstCombine.h#L29
[u-iciter]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L6199-L6230
[u-icpa]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L6286-L6291
[u-scfg]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L229-L270
[u-sroa]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SROA.cpp#L188
[u-ecsecap]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L76-L79
[u-ecsegen]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/EarlyCSE.cpp#L1066-L1103
[u-gvncap]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/GVN.cpp#L128-L150
[u-mdcap]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp#L74-L82
[u-licmcap]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L149-L160
[u-dsecap]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DeadStoreElimination.cpp#L126-L162
[u-lsrcap]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L6322-L6336
[u-fsp]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L629-L841
[u-early]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L643-L650
[u-gvnpos]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L776-L790
[u-ic1]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L677
[u-ic2]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L768
[u-ic3]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L801
[u-ic4]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L837
[u-globalcleanup]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1242-L1245
[u-inliner]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L972-L1071
[u-mssalist]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L753-L755
[u-optuses]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1307-L1341
[u-ensure]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2203-L2212
[u-msscap]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L76-L78
[u-msswalk]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2556-L2593
[u-mssaopt]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/MemorySSA.h#L329-L356
[u-baa]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1684-L1720
[u-uodepth]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueTracking.h#L52
[u-batchaa]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/AliasAnalysis.h#L608-L622
[u-dagrun]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L1845-L1962
[u-dagcomb]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L977-L983
[u-mbb]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineBasicBlock.h#L138-L152
[u-mop]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineOperand.h#L190-L192
[u-mri]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineRegisterInfo.h#L87-L89
[u-slot]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/SlotIndexes.h#L528-L566
[u-renumidx]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SlotIndexes.cpp#L193-L218
[u-lis]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveIntervals.h#L205
[u-lis2]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveIntervals.h#L356
[u-lis3]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveIntervals.h#L378
[u-lre]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveRangeEdit.h#L42-L222
[u-splitkit]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.h#L263
[u-rgq]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L420-L437
[u-rabase]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocBase.cpp#L88-L155
[u-trysplit]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1975-L2010
[u-liu]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveIntervalUnion.h#L42-L62
[u-icache]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InterferenceCache.h#L137
[u-bundles]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L240
[u-rgpres]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L341-L364
[u-fpstack]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FloatingPoint.cpp#L107-L120
[u-sv]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/SmallVector.h#L1166
[u-sv2]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/SmallVector.h#L1251
[u-dm]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/DenseMap.h#L12-L15
[u-dmprobe]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/DenseMap.h#L799-L829
[u-bump]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/Allocator.h#L65-L70
[u-scevbump]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L2595
[u-lisbump]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/LiveIntervals.h#L67-L68
[u-mf]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/MachineFunction.h#L327-L341
[u-recycler]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/Recycler.h#L37-L112
[u-sdalloc]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/SelectionDAG.h#L272-L289
[m-ids]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/module.rs#L1-L46
[m-func]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/module.rs#L284-L328
[m-create]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/edit.rs#L176-L199
[m-inst]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/module.rs#L70-L94
[m-slot]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/edit.rs#L201-L218
[m-insert]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/edit.rs#L316-L385
[m-users]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/module.rs#L525-L538
[m-adduse]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/edit.rs#L47-L78
[m-rauw]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/edit.rs#L489-L520
[m-log]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/module.rs#L133-L282
[m-touched]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/touched.rs#L31-L74
[m-reach]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/touched.rs#L154-L189
[m-fork]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/module.rs#L675-L699
[m-lsrclone]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/lsr.rs#L603-L617
[m-hoistclone]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/hoist.rs#L320-L325
[m-ctx]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/context.rs#L45-L92
[m-types]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/types.rs#L88-L108
[m-entry]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/passes.rs#L925-L1025
[m-get]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/passes.rs#L1295-L1468
[m-drop]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/passes.rs#L1526-L1563
[m-annot]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-analysis/src/manager.rs#L1088-L1177
[m-regs]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-analysis/src/manager.rs#L1343-L1452
[m-trait]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/passes.rs#L2105-L2132
[m-fold]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/fold.rs#L177-L195
[m-alg]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/algebraic.rs#L231-L258
[m-phiopt]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/phiopt.rs#L138-L162
[m-gvn]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/gvn.rs#L366-L435
[m-hoistloop]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/hoist.rs#L671-L743
[m-cap]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-analysis/src/memoryssa.rs#L1019-L1025
[m-rounds]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/pipeline.rs#L253
[m-every]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/pipeline.rs#L483-L532
[m-fixed]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-transforms/src/pipeline.rs#L993-L1069
[m-over]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/passes.rs#L2360-L2498
[m-mssa]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-analysis/src/memoryssa.rs#L237-L427
[m-mssaview]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-analysis/src/memoryssa.rs#L1666-L1689
[m-pv]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-analysis/src/alias.rs#L2746
[m-annotated]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-analysis/src/alias.rs#L3612-L3640
[m-escbefore]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/opt/llrm-analysis/src/alias.rs#L215
[m-dense]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/dense.rs#L1-L52
[m-bl]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/ir/llrm-mir/src/blocklist.rs#L19-L25
[l-insn]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/model/lir.rs#L106-L157
[l-block]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/model/lir.rs#L629-L677
[l-body]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/model/lir.rs#L929-L995
[l-idx]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/analysis/intervals.rs#L97-L143
[l-patch]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/analysis/intervals.rs#L209-L371
[l-post]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/backend/postings.rs#L600-L680
[l-ivupd]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/analysis/intervals.rs#L1040-L1214
[l-facts]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/backend/allocate.rs#L1576-L1801
[l-queue]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/backend/allocate.rs#L1993-L2585
[l-union]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/backend/liveunion.rs#L183-L303
[l-carve]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/backend/splitkit.rs#L586-L814
[l-peep]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/backend/peephole.rs#L301-L383
[l-jumps]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/crates/backend/llrm-core/src/backend/jumps.rs#L186-L212
[idle]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/idle.md
[regalloc]: https://github.com/ali-mosavian/llrm/blob/bd6e647f643dc71ff11a0ad7af8f00a008be2a82/docs/comparison/llvm/regalloc.md
