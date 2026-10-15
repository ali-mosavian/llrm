# Compile-time architecture: llrm at gcc's cost per unit of work

For contributors redesigning the compiler's core. It specifies the IR, the analysis manager, the pipeline and the backend that bring llrm's cost per unit of work to gcc's and LLVM's. It also gives the budgets that keep it there and the stages that get there. The measurements behind it are in [compile-time-review.md](compile-time-review.md); this document gives only the numbers its targets are derived from. It is measured at [a99bffc80](https://github.com/ali-mosavian/llrm/commit/a99bffc80c76da6fdffa295f6b77d5192d3ba275). "fn" means `D_DRAW_FACES` in qbq-llrm's `render/d_faces.c`, `-m32` unless stated.

TL;DR: llrm does the same work as gcc and LLVM, but each unit of it costs 10-100x more. A unit is a pass run, an analysis, a spill or split, or a clean-up visit. The cost is not in what is computed but in what is recomputed: whole functions are walked, cloned or re-derived for every edit. The fix is one rule enforced by the data structures. Every edit goes through one editor that records what it touched. Every fact has one owner that updates itself from that record. Every pass works on what changed since it last ran. With the per-unit budgets met, d_faces compiles in at most gcc's instructions at every level (-O0 231 M, -O1 636 M, -O2 1137 M, -O3 1413 M, -Os 944 M; today 512 M to 9.6 G). Code quality is held by gates that fail on any changed object (identical output) or any bench row that gets worse.

## 1. Targets

### 1.1 Where llrm stands

| | -O0 | -O1 | -O2 | -O3 | -Os |
|---|---|---|---|---|---|
| d_faces, llrm m32 (M instr) | 512 | 8281 | 8904 | 9571 | 8639 |
| d_faces, llrm m16 | 522 | 8399 | 10404 | 11044 | 10425 |
| d_faces, gcc / clang | 231 / 146 | 636 / 521 | 1137 / 671 | 1413 / 672 | 944 / 567 |
| corpus of 71 programs, llrm / gcc, net of an empty file, sum | 0.44 | 1.90 | 1.47 | 1.61 | 1.64 |
| corpus, llrm / gcc, net, geomean (worst) | 0.40 (1.0) | 1.64 (3.6 `x_life`) | 1.39 (2.7) | 1.33 (4.2 `nbody_single`) | 1.44 (3.3) |
| corpus, llrm / clang, net, geomean | 2.63 | 2.92 | 2.99 | 3.55 | 3.20 |

The corpus is the vsgcc programs, compiled `-c` with each compiler's flags. The net figures subtract each compiler's cost on an empty file: llrm 9 M, gcc 19 M with its assembler, clang 43 M with its dynamic loader. Small files hide the problem: llrm's cost per unit grows with function size. The fn is 3018 MIR instructions in 232 blocks at the start of the pipeline, 1196 at its end, and 1635 LIR instructions with 450 values when it reaches allocation. There llrm is 13x gcc and 16x clang. One caution: `levels_time.py` stops on `bench/bench_io.h` because it lists files as programs. The corpus above was compiled with its own `command()`, over the programs that `programs.sources()` lists.

### 1.2 Cost per unit

Measured at -O1 on the fn, from `LLRM_DEBUG=time` and `runs` for llrm and from the gcc and LLVM traces in the review (appendices A and B):

| unit | llrm today | gcc | LLVM | target |
|---|---|---|---|---|
| MIR pass run, per IR instruction of the function | ~19 k (28 M per run) | ~2 k (4 M per run) | ~220 (0.4 M per run) | **≤ 300** for a full run; a continuation run (section 5) costs ≤ 300 per instruction of its region |
| analysis computations per pass run | 2.8 | (TODOs: 57 for 154 passes) | 0.53 (0.26 on the fn) | **≤ 0.5** |
| full analysis computation, per IR instruction | ~260 (1.95 G / 2508 / ~3000) | points-to ~3 k per constraint | | **≤ 300** for function-wide facts; points-to and memory SSA **≤ 3 k**; an update costs ≤ that per instruction it reaches |
| spill or split event | ~4 M | spills in one LRA pass | ~0.2 M for the whole allocator; 28 k per `InlineSpiller::spill` | **≤ 0.2 M**, and ≤ 40 per LIR instruction of the edited range |
| split candidate costed | ~200 k | | 23 k | **≤ 30 k** |
| LIR clean-up, per block visit | ~8 k (0.98 G / 13 runs / ~40 sub-passes / ~240 blocks) | ~57 k per block per pass | ~5 k | **≤ 10 k per changed block**; after a pass's first visit, unchanged blocks are not visited |
| allocations of a function | up to 6 | 1 | 1 | **1** |
| LIR pipeline runs of a function | up to 3 | 1 | 1 | **1** |

LIR clean-up already costs what LLVM's does per block visit. Its gap is the number of visits: 13 runs × ~40 sub-passes × every block, against about 10 passes once. MIR's gap is both: 60x per pass run, mostly analyses recomputed inside the run.

### 1.3 Budget per job, and what it makes d_faces

Each job's budget is taken from whichever of gcc or LLVM is cheaper for that job on the fn (review section 2.2):

| job | evidence | -O0 | -O1 | -O2 | -O3 | -Os |
|---|---|---|---|---|---|---|
| front end to MIR (OW front end, HIR, HIR→MIR; verifiers off in release) | llrm 84 today, of which 20 is `hir verify` | 50 | 50 | 50 | 50 | 50 |
| MIR pipeline, passes and analyses | clang IR pipeline 169/275/285/207 | 10 | 250 | 400 | 450 | 300 |
| instruction selection | llrm 60-97, clang 51-56 | 60 | 60 | 60 | 60 | 60 |
| register allocation, liveness included | gcc IRA+LRA 54/105/129/148/78, LLVM ~40 | 40 | 100 | 100 | 100 | 100 |
| LIR clean-up (peephole, jumps, x87, coalesce, phi elimination) | gcc ~82, LLVM ~14 at -O1 | 40 | 80 | 80 | 80 | 80 |
| frame, layout, object | llrm ~30 | 30 | 30 | 30 | 30 | 30 |
| **d_faces total** | | **230** | **570** | **720** | **770** | **620** |
| gcc | | 231 | 636 | 1137 | 1413 | 944 |

The MIR budget is the unit targets times the units. At -O1 that is ~500 full pass runs and continuation runs over an average of ~1500 instructions at ≤ 300 per instruction, which is 225 M. Analyses at ≤ 0.5 computations per run add ~25 M on top of the passes' own updates.

Corpus targets: net of an empty file, the sum and the geomean of llrm/gcc are ≤ 1.0 at every level, no program is above 1.5x gcc, and the scaling slopes (`vsgcc/scaling.py`) are no steeper than gcc's on any axis. Today the steepest are 1.81 on joins and 1.80 on unrolled, against gcc's 1.41 and 1.04 ([pipeline-design note](pipeline-design.md)).

## 2. IR core

Today's structures, and what makes each cost more than O(edit):

| structure | today | cost |
|---|---|---|
| MIR block order | `Block.instructions: Vec<InstId>` [module.rs:82-86](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/module.rs#L82-L86) | insert, erase and move are a `position()` scan plus a shift, O(block) ([edit.rs:186-265](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/edit.rs#L186-L265)) |
| MIR use removal | `retain` over the value's use list [edit.rs:61-72](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/edit.rs#L61-L72) | `replace_all_uses_with` is O(U²) [edit.rs:474-483](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/edit.rs#L474-L483) |
| MIR speculation | `function.fork()` / `clone()` per numbering, per loop, per candidate ([gvn.rs:381](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/gvn.rs#L381), [hoist.rs:248-256](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/hoist.rs#L248-L256), [candidate.rs:139](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/candidate.rs#L139)) | O(function) per try; drops the change log, so every analysis marked before it recomputes ([module.rs:613-632](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/module.rs#L613-L632)) |
| LIR block | `Insns(Arc<[Arc<Insn>]>)`, edited by copying the block [model/lir.rs:640-653](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/model/lir.rs#L640-L653) | O(block) per edit; a new `Arc` per block per sub-pass ([backend/peep/walk.rs:507-516](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/peep/walk.rs#L507-L516)) |
| LIR instruction | `Insn` with `Vec`/`BTreeSet` fields: clobbers, requires, delivers, widths, operands as `Loc` inside `Semantics` [model/lir.rs:113-146](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/model/lir.rs#L113-L146) | a clone per rewrite; set operations on trees |
| LIR slots | `IndexMap<insn id, slot>` [analysis/intervals.rs:105](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/intervals.rs#L105), patched by comparing every block per round [analysis/intervals.rs:230-370](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/intervals.rs#L230-L370) | O(B) plus a hash per lookup |
| LIR uses | thread-local `Postings`, checked against every block by pointer per query [backend/postings.rs:618-637](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/postings.rs#L618-L637) | O(B) per query |
| LIR body | `LirBody` with per-body `IndexMap`s, copied by `with_blocks` [model/lir.rs:909-1010](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/model/lir.rs#L909-L1010) | O(values + edges) per carve and per peephole group |

### 2.1 One function layout for MIR and LIR

Both levels use the same shape. LLVM has `ilist` plus `Use` lists ([Use.h:98-106](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/Use.h#L98-L106), [BasicBlock.h:63](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/BasicBlock.h#L63)) and `MachineRegisterInfo` def/use chains ([MachineRegisterInfo.cpp:273](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/MachineRegisterInfo.cpp#L273), [MachineRegisterInfo.cpp:312](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/MachineRegisterInfo.cpp#L312)). llrm keeps its arenas of ids:

```
Function / LirFunction
  insts:   Vec<Inst>            stable InstId, never reused within a compile
  prev,next: Vec<InstId>        intrusive order; block.first/last
  block:   Vec<BlockId>         each instruction's block
  order:   Vec<u32>             gapped order number within its block (LLVM comesBefore)
  uses:    Vec<UseList>         per value: (user, operand index) with a back-index in each operand, O(1) removal
  blocks:  Vec<Block>           first, last, preds, succs (kept by the editor), version: u32
  log:     ChangeLog            every edit, with the old operand where one was replaced
```

- **Order.** Insert, erase and move are O(1). "a before b" compares order numbers. A block renumbers lazily when a gap runs out ([Instruction.cpp:334](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/IR/Instruction.cpp#L334), [BasicBlock.cpp:726](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/IR/BasicBlock.cpp#L726)).
- **Uses.** Each operand slot stores its position in the value's use list. Removal is swap-remove plus fixing one back-index. `replace_all_uses_with` is O(U).
- **Identity.** `InstId` and `ValueId` are stable for the whole compile; they are never compacted mid-pipeline, so a fact keyed by id survives every edit that does not erase that id. The dead-id overhead is bounded by recycling ids only at module boundaries, which renumber and publish a map.
- **LIR instructions** are MIR-shaped. An operand array of `{ kind: Vreg | Phys | Mem{base, index, disp} | Imm, def/use, tied }` replaces `Loc` inside `Semantics` (LLVM `MachineOperand`, [MachineOperand.h:48](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/CodeGen/MachineOperand.h#L48)). Clobbers are a register bitmask per class, not `BTreeSet<RegId>`. `requires`/`delivers` are operands with a fixed register. The slot index lives on the instruction.

### 2.2 One editor, one record

Every mutation goes through `Editor` (MIR today has `edit.rs`; LIR gets the same). The editor appends to the change log. llrm's log already names instructions and old operands ([module.rs:135-176](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/module.rs#L135-L176), [module.rs:625-632](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/module.rs#L625-L632)), which is finer than LLVM's `PreservedAnalyses` or gcc's TODO flags. A consumer asks for the digest since its mark:

```
Touched {                                   as built in #1744 (llrm-mir/src/touched.rs), plus `pointers`
  blocks:            BitSet<BlockId>        any instruction added, removed, moved or rewritten
  blocks_created, blocks_erased: BitSet<BlockId>
  cfg:               Vec<(BlockId, BlockId, Added|Removed)>   net edges
  values_added, values_removed, operands_changed: BitSet<ValueId>
  memory_added, memory_removed, memory_moved, memory_rewritten: BitSet<InstId>   loads, stores, calls;
                                            rewritten = a store or call whose operands changed in place
  parameters:        bool
  pointers:          BitSet<ValueId>        pointer-typed values added, removed or with changed operands, and the values
                                            of changed pointer constraints (stores and loads of pointers, calls passing
                                            them): what PointerValues' keep-or-re-solve rule reads (3.2)
}```

- `Function::touched_since(mark) -> Option<Touched>` is derived from the log on demand, so a consumer pays for the edits since its mark only. `None` means the mark no longer resolves (another lineage, or trimmed) and the consumer recomputes.
- **Interim, as of #1744** (to close in the next core PRs):
  - `Touched` is returned by value, not yet cached per mark;
  - classification is by the instruction's current opcode until the log carries old opcodes, which comes with the invertible log and checkpoint/rollback;
  - `pointers` needs types, so it is filled by `touched_since(mark)?.with_pointers(function, context)` (#1744): pointer- and aggregate-typed values added, removed or with changed operands, plus the pointer-typed operands and results of loads, stores and calls added, removed, moved or rewritten. The `pointers` epoch stays as the O(1) key;
  - there is no `globals` epoch yet: the editor has no `Context` to tell a global from an integer constant.
- **Speculation without copies.** `editor.checkpoint()` returns a mark, and `editor.rollback(mark)` replays the log backwards; the log already holds old operands and positions. Pricing a hoist, a numbering or a candidate transform edits the live function and rolls back if rejected. `fork`, and every `clone` used for pricing, go. A rolled-back edit leaves no `Touched` behind for consumers, because the log is truncated to the checkpoint.
- **Epochs.** Each function also keeps never-reset counters, bumped by the editor from each change: `Function::epochs() -> Epochs { cfg, memory, pointers, calls }`, and `globals` once the editor can see the `Context`. A module-level memo (GlobalsAA's and Summaries' per-body contributions) stores the epochs it read and is valid iff they are equal. That is an O(1) check, coarser than `Touched`. Function analyses use `update(prev, &Touched)`; memos that only need "did anything of this kind change in this body" use the epochs.
- **Liveness and intervals** (LIR):
  - per-block use/def and live-in/out as dense bitsets indexed by vreg, kept on the block with its version;
  - per-vreg live intervals as a sorted `Vec<Segment>`, computed once per function and edited by the allocator's edits (LLVM `LiveRangeEdit`, [LiveRangeEdit.cpp:55](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveRangeEdit.cpp#L55));
  - interference per physical register as an interval map with a version tag (LLVM `LiveIntervalUnion`/`IntervalMap`, [LiveIntervalUnion.h:46](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/CodeGen/LiveIntervalUnion.h#L46)).
- **What replaces what:**
  - `Arc<LirBlock>` identity becomes `(BlockId, version)`; the editor bumps the version on any edit to the block;
  - `IndexMap<id, _>` and `BTreeMap<id, _>` side tables become dense `IdMap`s;
  - `BTreeSet` liveness becomes bitsets;
  - thread-local `Postings` becomes the use lists.

### 2.3 The edit API contract (build first)

This section and section 3 are what the rest builds on. Owner: worst-rows; regalloc (LIR allocator) and through-memory (alias, memory SSA) build on them. Both MIR and LIR implement the same contract.

**Identity.**
- `InstId`, `ValueId` and `BlockId` are never reused while a function is being compiled. An erased instruction keeps its arena slot, marked erased, so that a rollback can restore it and a fact keyed by its id can tell it is gone.
- LIR instructions get the same arena ids. `Insn::id()`, a global counter given on first ask ([model/lir.rs:444-447](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/model/lir.rs#L444-L447)), and `Arc` pointer identity as a key both go.

**Mutations.** All go through `&mut Editor`. Today's MIR mutators are `edit.rs`'s `insert`, `move_to`, `move_run`, `set_operand(s)`, `replace_all_uses_with`, `replace_value`, `erase`, `erase_all`, `create_block`, `insert_block`, `erase_block`, `set_flags`, the attribute and parameter setters ([edit.rs:161-976](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/edit.rs#L161-L976)). LIR gets the same set over its instructions, plus `set_successors`. A field changed outside the editor is a bug the verifier reports.

**The log is complete and invertible.** Each `Change` ([module.rs:135-176](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/module.rs#L135-L176)) carries what undoing it needs:

| change | today | adds |
|---|---|---|
| `Inserted`, `Erased`, `BlockCreated`, `BlockErased` | position | — (an erased instruction stays in the arena) |
| `Moved` | new position, old block | old position (`from_next`) |
| `Operand`, `Operands` | old operands | — |
| `Rewritten` | id only | old opcode, flags or attributes when those changed (`set_flags`, `make_store_*`, attribute setters) |
| `Parameters` | kind only | the old parameter list |
| LIR | none | the same kinds, with `Successors { block, was }` |

**The digest classifies from the log.** The memory bit (a change to a load, a store or a call) and the pointer bit (a pointer-typed value, or a store, load or call that moves a pointer) are derived from each change's instruction and its old operands. The record therefore needs no field of its own for them; for an erased or rewritten instruction, the old opcode and operands it carries are what the classification reads.

**Marks and the digest.**
- `function.mark()` is a point in the log; `function.touched_since(mark) -> &Touched` is the digest of section 2.2, built once per (mark, log length) and cached.
- `clone()` of a function is legal only outside the pipeline (tests, the interpreter) and is counted. `fork()` goes: nothing replaces a function with a copy.
- The log is never trimmed while a mark older than its start is held. Each consumer takes its own mark, and the manager trims below the oldest live one. That replaces the fixed `1 << 17` trim ([module.rs:589-596](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/module.rs#L589-L596)).

**Speculation.**
- `editor.checkpoint() -> Checkpoint` and `editor.rollback(Checkpoint)` replay the log backwards to the checkpoint, then truncate it. The function compares equal (`PartialEq`) to what it was.
- Pricing passes edit the live function, price it, and roll back if rejected: gvn's two numberings, hoist's `_affordable`, the candidate passes' nested pipelines ([gvn.rs:381-424](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/gvn.rs#L381-L424), [hoist.rs:248-332](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/hoist.rs#L248-L332), [candidate.rs:139-160](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/candidate.rs#L139-L160)).
- A nested analysis computed during the speculation is dropped on rollback, because its mark is past the checkpoint.

**Cost contract.**
- insert, erase, move and set_operand are O(1) amortised; `replace_all_uses_with` is O(uses); rollback is O(changes undone).
- `touched_since` is O(changes since the mark).
- Every editor method increments `visited` by what it reads, so section 6's counted budgets can hold passes to it.

**Tests that come with it, each seen to fail first:**
- for every mutator, `touched_since` names exactly the blocks, values and memory instructions it changed;
- a random edit sequence followed by rollback to every checkpoint gives back an equal function;
- a fact updated from `Touched` equals the fact recomputed (the check-mode property), for Dominators and Registers first;
- the counted budget: no `clone` and no whole walk inside the editor.

## 3. Analysis manager

### 3.1 Semantics

**The protocol exists; the gap is coverage and history.** The tree already has it: `SKIPS` with `Depends::unaffected`, `INCREMENTAL` with `update`, `PreservedAnalyses`, and the check modes `LLRM_CHECK_REPLAY` and `LLRM_CHECK_PRESERVED` ([passes.rs](https://github.com/ali-mosavian/llrm/blob/ca31bc21d/crates/ir/llrm-mir/src/passes.rs)). worst-rows measured that on d_faces -O1 it leaves little to gain by itself (stage 1 below). What the spec adds is (a) update rules for the facts that have none (3.2), (b) declared `DEPENDS` and `OUTER_DEPENDS`, and (c) a history that survives a pass and a fork (`Touched`, epochs, marks). The trait below is the target shape of what exists, not a new interface.

The manager is LLVM's new pass manager with llrm's change log driving invalidation ([PassManagerImpl.h:137](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/PassManagerImpl.h#L137), [PassManagerImpl.h:172](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/PassManagerImpl.h#L172), [Analysis.h:111](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/IR/Analysis.h#L111)). Today's cache, `HashMap<TypeId, _>` per function, stays ([passes.rs:966-1088](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L966-L1088)). Every analysis implements:

```
trait Analysis {
    const DEPENDS: &[AnalysisId];                    // static, checked: run() may get only these
    fn run(fn, deps) -> Result;                       // full computation, counted
    fn update(prev, touched: &Touched, deps) -> Update // Kept | Patched(Result) | Recompute
    fn reads() -> Footprint;                          // which parts of Touched can affect it
}
```

- **Lazy.** Nothing runs until `get`. On `get`, the manager brings the result up to date: its dependencies first, in `DEPENDS` order, then `update(prev, touched since prev's mark)`. `Recompute` falls back to `run` and is counted.
- **Preserved** is what a pass declares unchanged (as today) plus everything whose `reads()` its `Touched` does not meet. A pass cannot break a result it did not touch the inputs of. That replaces today's `none().preserve::<Dominators>().preserve::<Loops>()`, which drops everything else on any edit ([hoist.rs:134](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/hoist.rs#L134), [gvn.rs:198-199](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/gvn.rs#L198-L199)).
- **No invalidate-all inside a pass.** fold, decide and jumpthread call `invalidate(none)` mid-run today ([fold.rs:175-177](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/fold.rs#L175-L177), [decide.rs:161-164](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/decide.rs#L161-L164), [jumpthread.rs:235-243](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/jumpthread.rs#L235-L243)). Their edits are in the log, and the next `get` updates.
- **One owner per fact.** A pass, a module analysis or isel never solves a fact the manager owns; it gets it. The `Unit` fallbacks that compute a missing fact privately go ([memory.rs:1048-1061](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/memory.rs#L1048-L1061)).
- **Dataflow analyses** (MIR and LIR liveness, escapes, memory SSA phis) are solved gcc df style ([df-core.cc:1265](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1265), [df-core.cc:1601](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-core.cc#L1601), [df-problems.cc:968](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/df-problems.cc#L968)):
  - per-block transfer sets are cached with the block's version;
  - `Touched.blocks` recomputes only those blocks' sets;
  - the solution is reset on the dirty blocks and the blocks their change can reach in the problem's direction, then solved with an RPO worklist while the rest keeps its values;
  - invariant: a block's transfer function changes only when one of its instructions changed, and a block outside the reset region cannot change value.
- **Module analyses** live in one `ModuleAnalyses` for the whole pipeline, not one per segment ([passes.rs:1882-1885](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L1882-L1885)), including the interprocedural pass's ([interprocedural.rs:351-367](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/interprocedural.rs#L351-L367)).
  - A per-body contribution is kept by the body's mark and redone only for edited bodies.
  - GlobalsAA and Summaries read the function owners' results instead of their own `Bodies` and `SummariesMemo` ([globalsaa.rs:317](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/globalsaa.rs#L317), [manager.rs:521](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L521)).
  - A changed summary invalidates the outer-reading results of the callers of that callee only, not of every body ([passes.rs:1524-1535](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L1524-L1535)).
- **Check modes are the correctness net.** Every `update` has `LLRM_CHECK_<FACT>`, which recomputes and compares (the tree has 30 such modes for kept caches). `tools/identical.py` runs with them on, and CI runs them over the corpus. A stage that adds an update rule adds its check mode in the same commit.

### 3.2 Every fact, its owner and its update rule

Columns: the edit record's parts are **B** blocks touched, **V** values added or removed, **O** operands changed, **M** memory instructions added, removed or moved, **E** CFG edges changed.

- *Today*: **none()** means dropped by every changing pass; **SKIPS** means revalidated over the log; **INCR** means updated.
- *Laziness*: all MIR facts are lazy on `get`; LIR facts become lazy once owned.

**MIR, per function**

| fact | defined | depends on | today | update rule |
|---|---|---|---|---|
| Dominators | [passes.rs:566](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L566) | — | INCR | kept unless E; on E, redo the subtrees under the changed edges (as today) |
| Loops | [passes.rs:598](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L598) | Dominators | none() | kept unless E; on E, rebuild the loops whose header, latch or exit edges changed |
| Shape (dominance, loops, reaching memo) | [cfg.rs:536](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/cfg.rs#L536) | Dominators, Loops | follows both | derived; clears the reaching memo on E. Its ~50 direct `Shape::of` calls go |
| ScalarEvolution | [passes.rs:615](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L615) | Loops | none() | drop erased values; redo recurrences reached by def-use from O/V; full on E |
| AssumptionCache | [manager.rs:1341](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1341) | — | none() | add or remove assume calls in V; full on E |
| ExposedFrames | [manager.rs:1050](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1050) | outer | SKIPS | drop erased allocas; recheck allocas whose def-use reach meets O/V |
| PointerValues | [manager.rs:1076](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1076) | Shape, ExposedFrames; outer: Summaries (captures, returns) | SKIPS; flow-sensitive through per-block cell maps | **one flow-insensitive solve per body**: a worklist over SSA values and constraints (Andersen, as gcc's `solve_graph`, [tree-ssa-structalias.cc:2711](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-structalias.cc#L2711)); one abstract location per object, no per-block maps. An edit that touches no pointer-typed value and no pointer constraint (`Touched.pointers` empty) keeps it; otherwise one re-solve of the body, counted |
| Pointers (escapes) | [manager.rs:1097](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1097) | PointerValues, AssumptionCache; outer: Summaries (captures) | SKIPS | escapes per query, cached, invalidated per instruction in O/M (LLVM `EarliestEscapeAnalysis`, [BasicAliasAnalysis.cpp:213](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L213)). A query that needs a cell's contents goes through the memory SSA walker with its cache, not through the solve |
| Registers (`consts::known`) | [manager.rs:1213](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1213) | Shape, AssumptionCache, ExposedFrames | INCR | as today: values reached by def-use from O/V, plus table loads |
| DemandedBits | [manager.rs:1321](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1321) | — | none() | drop erased; redo operands of O/V instructions backward to a fixed point |
| Counted | [manager.rs:1385](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1385) | Shape, Registers | INCR per loop | as today |
| Followers | [manager.rs:1358](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1358) | Shape, Registers | none() | per loop, as Counted |
| DominatedEdges | [manager.rs:1630](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1630) | Registers, Shape | INCR | as today |
| Bounded | [manager.rs:1689](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1689) | Counted, DominatedEdges | INCR | as today |
| Annotated (a MemRef per access) | [manager.rs:1128](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1128) | Pointers, Registers, Counted, DominatedEdges, Bounded | none(), no update | per access: drop erased; redo accesses whose address is reached from O/V, and moved accesses whose block facts differ; full on E |
| CallEffects | [manager.rs:1169](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1169) | PointerValues, Summaries | SKIPS | per call: drop erased; redo calls whose arguments are reached from O/V; the callers of a changed summary |
| Writes | [manager.rs:1519](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1519) | CallEffects | SKIPS | applies CallEffects' delta |
| Accesses (memory SSA) | [memoryssa.rs:237](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/memoryssa.rs#L237) | Annotated, CallEffects | INCR for moves only | updater: insert, remove and move accesses for M; redo clobbers below them in their blocks (LLVM `MemorySSAUpdater`, [MemorySSAUpdater.cpp:307](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSAUpdater.cpp#L307), [MemorySSAUpdater.cpp:1290](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Analysis/MemorySSAUpdater.cpp#L1290)); full on E |
| ThroughMemory (`known_walked`) | [manager.rs:1600](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1600) | Writes, Annotated, Registers | none() | as Registers, plus loads whose clobber changed under M |
| FloatFacts | [manager.rs:1565](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L1565) | ThroughMemory, Annotated | none() | float values reached from O/V; full on M or E |
| Published | [observers.rs:133](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/observers.rs#L133) | Pointers | none() | per-instruction contributions with counts; redo M/O instructions |
| TerminalSites | [noreturn.rs:195](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/noreturn.rs#L195) | Declarations | none() | add or remove calls in V; full on a declarations change |
| Products (frequencies, trips, odds) | [lsr.rs:406](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/lsr.rs#L406) | Registers, Shape, Counted | INCR | the one owner of MIR block frequencies; profit's private `_frequencies` go ([profit.rs:348](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/profit.rs#L348)) |
| MIR liveness (no owner today) | [liveness.rs:375](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/liveness.rs#L375) | Shape | per call (4 callers) | owned; dataflow rule of 3.1 |

**Points-to solve strategy, decided.** The solve is flow-insensitive per body, as gcc's, and the flow-sensitive precision is answered per query. through-memory measured the variant before the decision. Its solve has no per-block cell state, loads from objects, and does not order escapes. It makes 9.9% of pointer-value facts coarser and changes 23% of solves, but the output barely moves:
- 21 of 680 corpus objects change, all of them QCport modules, by at most +60 B on a 28 KB module;
- the 66 vsgcc programs are identical at every level;
- 1 of 424 bench rows is worse (parity/qmove, c -Os, +3.5%).

**Outer reads.** Today `READS_OUTER` is true by default ([passes.rs:276](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L276)): every analysis but Dominators, Loops, ScalarEvolution, Shape and DemandedBits is dropped whenever any module result moves ([passes.rs:1524-1535](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L1524-L1535)). Each analysis instead declares `OUTER_DEPENDS`, the module facts it reads and the keys it reads them by (a callee's summary, a global's declaration). The `Outer` accessor records reads in check builds and fails on an undeclared one. A changed module fact then invalidates only the results that read its changed keys. The known reads are in the table above: Summaries for CallEffects, PointerValues and Pointers; Declarations for ExposedFrames and TerminalSites. The owners complete the list in stage 1.

**What the edit record carries for these rules.** Each change records its kind, the instruction, the block, the new position and the old one (for `Moved`), and the old operands, opcode or flags (2.3). The digest classifies the memory bit: a change to a load, a store or a call is in `Touched.memory`.

**MIR, module and program**

| fact | defined | depends on | today | update rule |
|---|---|---|---|---|
| Declarations, CalleeEffects, CallRegisters, GlobalSizes | [passes.rs:535](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L535), [passes.rs:418](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L418), [passes.rs:442](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L442), [passes.rs:472](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L472) | globals | drop + `unchanged` | entries of edited globals only |
| TypeAncestry | [passes.rs:499](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/passes.rs#L499) | metadata | by length | extend with appended nodes |
| CallGraph | [callgraph.rs:303](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/callgraph.rs#L303) | bodies | dropped | edges of bodies with calls in V or a callee in O |
| GlobalsAA | [globalsaa.rs:325](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/globalsaa.rs#L325) | per-body Pointers (from the function owner) | per-body memo of its own | contributions of edited bodies |
| Summaries | [manager.rs:267](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L267) | GlobalsAA, per-body facts from the owners | memo of its own; `brought_up` | re-summarise edited bodies; propagate to callers of changed summaries |
| Callbacks | [manager.rs:208](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L208) | GlobalsAA, Summaries | while both are the same `Rc` | recompute when either changes |
| ProgramGlobals, ProgramSummaries | [globalsaa.rs:506](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/globalsaa.rs#L506), [manager.rs:808](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-analysis/src/manager.rs#L808) | every module | cleared | per edited module |

**LIR** (all move into one LIR manager with the MIR semantics)

| fact | defined | depends on | today | update rule |
|---|---|---|---|---|
| Graph, LoopDepths, EdgeBundles | [analysis/graph.rs:22](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/graph.rs#L22), [analysis/graph.rs:64](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/graph.rs#L64), [backend/spillplacement.rs:84](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/spillplacement.rs#L84) | labels, successors | `Kept` check is O(B) per call ([analysis/facts.rs:54-77](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/facts.rs#L54-L77)); `loops::loops` uncached at 10 sites | kept unless E; patched per changed edge |
| Frequency | [analysis/frequency.rs:124](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/frequency.rs#L124) | Graph, odds, trips | `Kept` | a block added on an edge takes the edge's frequency; full only when loops change |
| Slots | [analysis/intervals.rs:103](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/intervals.rs#L103) | instructions | ring by identity; patched over all blocks per round | stored on the instruction; a new instruction takes a midpoint; a block renumbers when its gap runs out ([SlotIndexes.h:531](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/CodeGen/SlotIndexes.h#L531), [SlotIndexes.cpp:172](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SlotIndexes.cpp#L172)) |
| Def/use lists (replace `Postings`, `Occurrences`) | [backend/postings.rs:93](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/postings.rs#L93), [analysis/occurrences.rs:45](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/occurrences.rs#L45) | instructions | thread-local, O(B) check per query | kept by the editor; never recomputed |
| Liveness (replaces live rows, lanes `live_into`, `dead_at_exit`) | [backend/allocate.rs:833](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/allocate.rs#L833), [backend/liveness.rs:331](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/liveness.rs#L331), [backend/liveness.rs:410](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/liveness.rs#L410) | Graph | rebuilt per call (8, 3 and 12 call sites) | one owner with the dataflow rule of 3.1; lanes are a refinement of the same per-block sets |
| Intervals | [analysis/intervals.rs:1032](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/analysis/intervals.rs#L1032) | Slots, Liveness, Frequency | ring, edit from the last answer, full past a third touched | computed once; values in V/O recomputed from their own uses and defs (LLVM `LiveIntervalCalc`, [LiveIntervalCalc.cpp:41](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/LiveIntervalCalc.cpp#L41)); no size cutoff |
| Allocator facts: masks, widths, hints, classes, sibling and fold prices | [backend/allocate.rs:1619-1808](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/allocate.rs#L1619-L1808) | Intervals, Slots | rebuilt per round from carried parts; full on a new epoch | for the edit's values only; classes counted per value, not compared whole |
| Interference | [backend/liveunion.rs:190](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/liveunion.rs#L190) | Intervals, assignment | per-register holder lists, lazily indexed | interval map per register with a version; `InterferenceCache` per (register, block) on top |

Callers that must ask an owner instead of computing: `cfg::Shape::of` (~50 sites), LIR `loops::loops` (10), `exposed_frames` (11), `consts::known` (8), `live_rows_by` (8), widths and masks outside the allocator (6 each), isel's own Shape, ExposedFrames and Registers ([backend/isel.rs:879-886](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/isel.rs#L879-L886)). The longest cascade today is Shape → Registers → Counted/DominatedEdges → Bounded → Annotated → Accesses, ThroughMemory → FloatFacts. Annotated has no update rule, so a drop anywhere before it rebuilds everything after it; giving Annotated and Accesses update rules breaks the cascade.

## 4. Backend

### 4.1 One pipeline, one allocation

Selection runs once. Then each LIR phase runs once, in today's order without the second route: phi elimination, two-address, coalescing, allocation, parallel copies, prologue, peephole, loop slots, scheduling, jumps ([flow.rs:66-139](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/flow.rs#L66-L139)).

The allocator is LLVM's greedy design, which llrm's already resembles ([backend/allocate.rs:2043-2393](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/allocate.rs#L2043-L2393)):
- **Queue.** A priority queue by size and class, stages Assign → Split → Spill → Done with no return. A range's stage only grows, so there is no global retry ([RegAllocGreedy.cpp:2422](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2422), [RegAllocGreedy.cpp:2516](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2516)).
- **Eviction.** Cascade numbers and a weight bound ([RegAllocGreedy.cpp:580](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L580)).
- **Region split.** Edge-bundle placement over the blocks where the value is live, priced through an interference cache per (register, block). The cache is validated by the union's version and reused across candidates ([RegAllocGreedy.cpp:1063](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1063), [InterferenceCache.cpp:131](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InterferenceCache.cpp#L131)). It replaces the gather-and-sort per register ([backend/splitkit.rs:1081-1103](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/splitkit.rs#L1081-L1103)). The block frequency map is a per-function fact, not rebuilt per split ([backend/spillplacement.rs:247](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/spillplacement.rs#L247)).
- **Local edits.** A spill or split returns its `Touched`. Only the intervals of the values it added or removed are computed, from their own uses and defs ([InlineSpiller.cpp:1285](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/InlineSpiller.cpp#L1285), [SplitKit.cpp:1502](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/SplitKit.cpp#L1502)). Masks, widths, hints and classes are updated for those values only. Nothing walks the function per round: `ranges::changes`, `Indexes::patch` over all blocks, `_wanted` over every hint and `gone` over every placed value all go ([backend/allocate.rs:2397-2582](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/allocate.rs#L2397-L2582)). Bridge blocks take slots inside a gap.
- **Recolouring** asks whether every holder has another free register before it moves anything ([allocation-algorithm note](../optimizations/allocation-algorithm.md), part 4).

### 4.2 Each candidate becomes a decision in the run

Each replacement is priced with the executed-work model where the decision is made ([single-allocation note](../optimizations/single-allocation-decisions.md)):

| today | decision in the single run |
|---|---|
| `Whole` (no splitting) | a split is priced with its border copies after hinting, and taken only if its pieces' cost plus copies is below the unsplit spill cost (LLVM `calculateRegionSplitCost`) |
| `Scoped` (protected loop copies) | a loop-local piece, weighted at the loop's frequency, is a split candidate at the Split stage; eviction prefers lighter holders over it (gcc IRA's regions done inside one run) |
| `Retainable` (one base kept) | spill weight charges the reload plus the extra instruction of an unfoldable address use (LLVM `weightCalcHelper`, [CalcSpillWeights.cpp:160](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/CodeGen/CalcSpillWeights.cpp#L160)) |
| `Addressed`, `Unfolded` | at spill time, fold or unfold per use by local cost (X86 `unfoldMemoryOperand`) |
| route B (`SsaSpill` first) | SsaSpill's pressure victims become an input: their spill weight is lowered by the pressure excess they relieve, so the queue takes them first where pressure is over the register count |

### 4.3 Recovering the rows, and the gate

One allocation and one route are the default from stage 0b, by the user's decision. Stage 0b records, per row, what dropping best-of-six cost; on the fn it was +1.2% static instructions at -O1 to -O3 and +2.5% instructions with +4.3% memory operands at -Os (review 2.2). The in-run decisions of 4.2 are the follow-up that recovers those rows. Each decision lands on its own, and the gate compiles every corpus object with and without it and reports per row.
- **m32:** the vsgcc table (`run.sh`, `table.py`). Executed instructions, memory operands and code bytes per program and level for llrm-single, llrm-today, gcc and clang.
- **m16:** `tools/bench` rows against `expected.toml`, and the DOS compilers' rows ([vs-borland-watcom](../comparison/vs-borland-watcom.md)); the m32-vs-m16 requirement ([m32-vs-m16](../comparison/m32-vs-m16.md)).
- **Landing rule for a decision**, at -O1, -O2, -O3 and -Os, m16 and m32:
  - no row has more executed instructions or memory operands than without it;
  - no -Os row has more code bytes;
  - no row where llrm is at or below min(gcc, clang) goes above it.
- **Done** when no row is worse than best-of-six was. The rows still short of that are named in each decision's PR.

Where today stands at m32 on this tree (`run.sh`, executed instructions, llrm over the other compiler, geomean with the worst row in brackets):

| | -O1 | -O2 | -O3 | -Os |
|---|---|---|---|---|
| 30 bench programs, over gcc | 0.63 (1.18 recfloat) | 0.75 (1.22 nbody_fixed) | 0.71 (1.38 nbody_fixed) | 0.69 (1.21 nbody_fixed) |
| 30 bench programs, over clang | 0.84 (2.85 recfloat) | 0.72 (1.81 matmul) | 0.69 (1.93 tile) | 0.71 (1.30 lru) |
| 41 x_ kernels, over gcc | 0.88 | 0.91 | 0.93 | 0.86 |
| 41 x_ kernels, over clang | 1.07 | 1.11 | 1.06 | 0.95 |

The gate holds every row at today's value or better. Rows above 1.0 today are the code-quality backlog; they are not this redesign's to fix and must not grow.

## 5. Pipeline

### 5.1 Fixed order once, with in-pass worklists

Every pass still takes MIR and returns MIR. The editor and the manager are how it does so, not a new interface between passes. The pass list keeps today's order ([pipeline.rs:449-495](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/pipeline.rs#L449-L495)). Each pass runs once in that order, iterating internally with a worklist (LLVM InstCombine, [InstructionCombining.cpp:5126](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5126); gcc FRE). Loop passes take loops from a loop worklist (LLVM `FunctionToLoopPassAdaptor`, [LoopPassManager.cpp:197](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LoopPassManager.cpp#L197)).

### 5.2 What replaces the rounds: a change-driven continuation

Today `Fixed::fixed` reruns the whole list until a round changes nothing ([pipeline.rs:948-1009](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/pipeline.rs#L948-L1009)). On the fn that is 5 rounds, 101 pass runs and 0.88 G of runs that changed nothing. Rounds 3-5 changed 7 things for 0.64 G. A hard cap of two rounds made bench rows worse, so the rewrites of the later rounds are kept. What changes is how they are found.

- Every pass declares `reads()`, a footprint of what its decisions depend on (blocks, values, memory, CFG; the `footprint` bits exist, [footprint.rs:64-88](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/ir/llrm-mir/src/footprint.rs#L64-L88)). It also implements `run_on(region)`, where a region is a set of blocks or loops.
- After round 1, the manager keeps per pass a dirty region: the part of the function whose `Touched` since that pass's last run meets its `reads()`, closed over def-use to the instructions those edits reach.
- The manager continues in pipeline order, running each pass with a non-empty dirty region on that region only, until every region is empty. A pass with no `run_on` uses the whole function, and the counters (section 6) report it.
- **Same output, by an invariant checked in CI.** A pass run on region R, when nothing outside R changed since its last full run, makes the same edits as a full run. `LLRM_CHECK_FIXPOINT` runs every pass in full after convergence and fails if any changes something; that is today's last round, run only as a check. It joins the check modes that `tools/identical.py` turns on.
- **Cost.** O(changes) after round 1, instead of O(rounds × function). On the fn, the 196 + 21 + 18 runs of rounds 3-5 become runs over the regions of 7 changes.
- The convergence check that prints the body to text per round ([pipeline.rs:964](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/pipeline.rs#L964), [pipeline.rs:1004](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/opt/llrm-transforms/src/pipeline.rs#L1004)) moves into the check mode.

## 6. Budgets, as counted tests

The budgets are counts, not times, so a test can assert them exactly. `llrm-support` gets always-on counters: an integer increment at each site, read by tests and by `LLRM_DEBUG=counts`. Today's `debug::counted` only works with the `time` channel on ([debug.rs:343](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/support/llrm-support/src/debug.rs#L343)), and `assemble::machinings`/`routes`/`selections` are the only counters that exist without it ([backend/assemble.rs:688-712](https://github.com/ali-mosavian/llrm/blob/a99bffc80c76da6fdffa295f6b77d5192d3ba275/crates/backend/llrm-core/src/backend/assemble.rs#L688-L712)).

| counter | site | budget asserted on fixtures (the fn, the scaling axes at N and 2N, the bench) |
|---|---|---|
| `function_clones` | `Function::clone`/`fork`, `LirBody` copies | 0 in the pipeline |
| `allocations`, `lir_pipeline_runs` | allocator entry, `flow::machine_with` | 1 per function |
| `analysis_computations / pass_runs` | manager `run` vs `update`/hit | ≤ 0.5 |
| `full_computations[A]` per function | manager | points-to ≤ 2, memory SSA ≤ 2, `Annotated` ≤ 2, liveness ≤ 1, loops ≤ number of CFG-changing passes |
| `visited` per pass run | editor and iterators: instructions a pass reads | ≤ 2 × function size on a full run; ≤ 2 × region size on a continuation run |
| `whole_walks` per continuation run | `Function::walk`, `body.insns()` | 0 |
| allocator `visited` per spill/split | editor | ≤ 40 × instructions in the edited range + 4 × blocks touched |
| allocator `full_interval_builds` | intervals | 1 per function |
| clean-up `block_visits` per sub-pass | peephole and jumps drivers | ≤ blocks on the first visit, then ≤ dirty blocks |
| slopes | `scaling.py`, counted instructions | ≤ gcc's slope + 0.05 per axis |
| `HINT_DEPTH` | `allocate::HINT_DEPTH`, the copies a fixed register's hint reaches a value through | 128 (a named limit, as gcc's `--param`s; the corpus reaches 74 at most, 99.99% of 141,519 walks ≤ 17); `LLRM_CHECK_WANTED` asserts the bounded walk equals the unbounded one where the walk ended within it |

A budget test fails like any other test, with the counter, the budget and the fixture named. Each budget is set at the end of the stage that reaches it, and never raised without a measured reason in the PR. The existing relative gate (`measure` in `tools/gate/tiers.toml`, against the merge-base) stays.

## 7. Migration

Each stage lands alone. Stages marked *identical* must pass `tools/identical.py` (every program and QCport module at every level, byte-identical, with the `LLRM_CHECK_*` modes on). Stages that change output must pass the gate of 4.3. The expected d_faces -O1 after each stage is projected from the measured component it removes; the components are in review sections 2-7.

Measured on main `c04886c2e`, d_faces -O1, `-m32` (instructions):

| | default | one route | one allocation | both |
|---|---|---|---|---|
| llrm | 6808 M | 5493 M | 5776 M | 4863 M |

With one allocation the 4.86 G splits into these pools, from `LLRM_DEBUG=time` own work:

| pool | M | what is in it |
|---|---|---|
| analyses | 1945 | points-to family 942 (points-to 200, pointer-values 207, GlobalsAA 212, Summaries 323); call-effects 178, through-memory 168, bounded 117, float-facts 103, annotated 89, dominated-edges 78, registers 66, counted 47 |
| MIR passes | 1706 | hoist 366, gvn 193, decide 165, interprocedural 127, stamp 118, algebraic 109, indvars 96, jumpthread 90, lsr 87, dse 67, promote 55 |
| allocator | 543 | split placed 155, regalloc own 46, split carving 29, spill 25, facts 23, recolor 20 |
| LIR passes | 510 | peephole 177, jumps 82, floatassign 71, coalesce 52, phielim 26, floatalloc 24 |
| front end, isel, frame | ~160 | |

| # | stage | output | owner | pool it works on (measured) | what it does not count on |
|---|---|---|---|---|---|
| 0 | Counters and budget tests in report mode; corpus baseline recorded (vsgcc rows, m16 bench, compile cost); `levels_time.py` fixed to list programs only | identical | worst-rows | — | — |
| 0b | **One allocation and one route by default** (the user's decision; regalloc's flag-drop PR) | changes; rows recorded for 4.3 | regalloc | −1.95 G measured on main (6.81 → 4.86 G) | — |
| 1 | **The core (2.3 and 3.1), MIR and LIR**: invertible log, stable ids, `Touched`, epochs, checkpoint/rollback replacing `fork` and pricing clones; `DEPENDS`/`OUTER_DEPENDS`; check modes | identical | worst-rows | non-pointer equal-result recomputations after `dead`: ThroughMemory, FloatFacts, Annotated, Assumptions, **40-60 M** (about 1% of 4.86 G), with prune rules | the 124 M of GlobalsAA's equal-result runs: skipping bodies whose edits do not affect them saved 27 M, and a persistent `ModuleAnalyses` saved 0, because the declarations legitimately change between runs |
| 2 | LIR representation on the core (arena, operands, slots on instructions, use lists); allocator local edits, interference cache, recolouring pre-check | identical | regalloc | allocator 543 M, of which `split placed` is 155 M | — |
| 3 | Points-to as decided in 3.2: one flow-insensitive solve per body, re-solved only when `Touched.pointers` is non-empty; escapes per query; cell contents through the memory SSA walker; memory SSA updater; `Annotated` per access; GlobalsAA and Summaries read the owner | changes (21 QCport objects, 1 bench row, measured); gated by 4.3 | through-memory | the points-to family, 942 M. gcc's whole `compute_may_aliases` is 16 M and LLVM's BasicAA 9 M | — |
| 4 | MIR order and uses O(1); update rules for the rest of 3.2; every `Shape::of` and private solve asks its owner | identical | worst-rows | the remaining analyses, ~1.0 G, the non-pointer rows above. Update rules remove only their equal-result part (stage 1's 40-60 M); the rest is each analysis's own per-run cost, a stage-8 item | a gain from the rules alone: worst-rows measured stages 1 and 4 together at about 1-2% |
| 5 | Regional entry points for passes (`run_on(region)`) and the change-driven continuation replacing rounds; `LLRM_CHECK_FIXPOINT` | identical | slopes | the idle work of the optimizer rounds, ~0.87 G, mostly the confirmation run after a pass's own change. Only a per-pass regional entry point removes that run | rounds without regional entry points |
| 6 | LIR liveness owner with dirty blocks; peephole and jumps on dirty blocks; floatassign exits before rebuilding | identical | regalloc | LIR passes, 510 M | — |
| 7 | The in-run decisions of 4.2, one at a time, recovering the rows 0b changed | changes, gated by 4.3 | regalloc | — (compile cost unchanged or slightly up) | — |
| 8 | Per-pass budgets: each pass brought to ≤ 300 instructions per IR instruction per full run. First `hoist` (366 M own, 564 M inclusive): invariant-run 254 M over 19.8 k `_movable` calls, affordable (clone and price) 139 M, memory SSA 138 M, frequency 28 M. Then gvn, decide, interprocedural, stamp, lsr; isel's constant factor; `hir verify` off in release | identical | each pass's owner (worst-rows takes hoist) | MIR passes, 1706 M, and the analyses' own per-run cost | — |

**What the core is for.** Stages 1 and 4 are small by themselves: the equal-result recomputations that `LLRM_WHY` finds are mostly the pointer ones (stage 3) and GlobalsAA's, which are legitimate recomputations. The core's value is that stages 2, 3, 5 and 8 need it: speculation without copies (stage 8's hoist and gvn), an edit record for the allocator (2), a digest for the points-to solve's keep-or-re-solve rule (3), and the dirty regions of stage 5. It lands first because the others depend on it, not because it pays for itself.

**The equal-result pool, measured.** `LLRM_WHY` records every recomputation of a function or module analysis, the pass that caused it, and whether the result came out the same. On d_faces -O1 with one allocation (2449 records):

| outcome | count | est. M |
|---|---|---|
| `first` (nothing held) | 840 | 438 |
| `same`, `same-untouched` (recomputed to an equal result) | 459 | 327 |
| `kept-rerun-same`, `-untouched` (kept for update, rerun, equal) | 398 | 229 |
| `replayed` (updated incrementally) | 474 | 237 |
| `diff`, `kept-rerun-diff` (a real change) | 278 | 211 |

`est. M` is the count times that analysis's mean own cost per call, so it is an estimate. Equal-result recomputations total ~0.56 G: GlobalsAA 124 M, ThroughMemory 88, CallEffects 75, PointerValues 64, Annotated 47, points-to 44, FloatFacts 38, mostly after `dead` and `fold`. worst-rows measured how much of it a rule can remove: GlobalsAA's 124 M cannot be captured (above); the pointer ones belong to stage 3; the rest, ThroughMemory, FloatFacts, Annotated and Assumptions after `dead`, is 40-60 M with prune rules.

After stage 8, d_faces is at the section 1.3 budgets at every level, and the counted tests of section 6 hold. Each stage reports its measured d_faces -O1 in its PR; this document gives no cumulative projection beyond stage 0b's measured 4.86 G, because the pools overlap and two of the projections made earlier did not survive measurement.

The order puts the core first. Stage 0b is done by the user's decision. Stage 1 is what every other stage builds on, and its section (2.3, 3.1) is written so it can start before the rest. Once 1 lands, stages 2 (regalloc), 3 (through-memory), 4 (worst-rows) and 5 (slopes) touch disjoint crates and can run in parallel. Stage 6 needs 2. Stage 7 needs 2 and 6. Stage 8 needs 4 and 5.
