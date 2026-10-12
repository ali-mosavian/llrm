# Analyses

For compiler developers. Every analysis in `crates/opt/llrm-analysis` and the pass manager's own: the one fact it owns, how it is computed, who asks it, and what keeps its answer valid. A pass asks the analysis that owns a fact and does not recompute it. The passes that ask are in [passes](passes.md); the machine phases that read a few of these are in [backend](backend.md).

```
  llrm-mir Function / Module / Program
        |
        v
  +-----------+   +----------------+   +----------------+
  | cfg:Shape |-->| consts, ranges |-->| induction      |
  | ssa, occ. |   | guards, floats |   | peelsize, jam  |
  +-----------+   +----------------+   +----------------+
        |                 |                    |
        v                 v                    |
  +-----------------------------+              |
  | memory:Unit, regions, alias |<-------------+
  | globalsaa, memoryssa, avail |
  +-----------------------------+
        |   manager.rs: cached entries (Analyses), invalidated per pass
        v
  +--------------------------------------------------+
  | llrm-transforms passes: gvn dse promote hoist    |
  | loopmotion lsr indvars unroll peel decide ...    |
  +--------------------------------------------------+
        |
        v
  +--------------------------------------------------+
  | backend isel / jumps: shape, consts, ranges,     |
  | trip counts, branchprob, frame escape            |
  +--------------------------------------------------+
```

Arrows point from an analysis to the analyses and passes that read it. `consts`, `ranges` and `induction` take a `memory::Unit`, so they sit on the memory vocabulary too.

## The manager

`manager.rs` makes the analyses entries of `llrm_mir::passes::Analyses` (function level) and `ModuleAnalyses` (module level). A pass asks with `analyses.get::<X>(...)`. The manager computes once and drops what a pass does not preserve (`.preserve::<X>()`, as in `addresssink`). An entry builds a `Unit` over its function with `Unit::within`.

Function entries: `ExposedFrames`, `PointerValues`, `Pointers`, `Annotated`, `CallEffects`, `Registers`, `DemandedBits`, `AssumptionCache`, `Followers`, `Counted`, `Writes`, `FloatFacts`, `ThroughMemory`, `DominatedEdges`, `Bounded`. Module entries: `GlobalsAA`, `Summaries`, `Callbacks`; program entry: `ProgramSummaries`. `Ranges` and `Held` are bundles of entries a pass asks together.

A result survives a change when the entry says so:

- `SKIPS` with `depends`: the change log is filtered against what the result reads (`ExposedFrames`, `PointerValues`, `Pointers`, `CallEffects`, `Writes`); a change it cannot see leaves the result standing.
- `INCREMENTAL` with `update`: `Counted`, `DominatedEdges` and `Bounded` redo only the loops or blocks that [`reached_by`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/manager.rs#L959) names. A change to the CFG derives them afresh.
- `covers`: `GlobalsAA` and `Summaries` keep a stale result that covers the fresh one, so the callers of a changed body need not be redone. `Callbacks` is rebuilt only when it was made from other `GlobalsAA` or `Summaries` results.
- `READS_OUTER = false`: `DemandedBits` outlives a change to the module.

Every other entry is recomputed on its next ask.

## The pass manager's own analyses

`crates/ir/llrm-mir/src/passes.rs` defines the contract and the analyses that read no `llrm-analysis` module.

| Entry | Level | Owns | Invalidated |
|---|---|---|---|
| [`Dominators`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L512) | function | The dominator tree; read through `cfg::Shape`. | a pass that changes the CFG; one that only moves instructions declares `.preserve::<Dominators>()` |
| [`Loops`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L529) | function | Natural loops (`LoopInfo`). | as `Dominators` |
| [`ScalarEvolution`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L546) | function | Add recurrences over `Loops`. | with `Loops` |
| [`CalleeEffects`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L364) | module | What each function does to memory, from its declaration's attributes alone. | a changed declaration |
| [`CallRegisters`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L388) | module | How many registers a call to each function keeps, as the target says of it by name. | a changed declaration |
| [`GlobalSizes`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L418) | module | Each global variable's size in bytes. | a changed global |
| [`TypeAncestry`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L445) | module | The module's `!tbaa` type tree; read by `regions::typed_apart`. | changed metadata |
| [`Declarations`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L481) | module | Every global as its declaration, by id: what a function analysis may read of the others. | a changed declaration |

`Outer` ([`passes.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L642)) is the read-only proxy a function analysis reads its module through: metadata, globals, the program, and the module analyses the pass manager `require`s (`GlobalsAA`, `Summaries`, `Callbacks`). `ModuleAnalyses` ([`passes.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L1211)) computes a module analysis once, keeps a dropped result and reuses it when `unchanged` says the module still reads the same, and records the bodies reported edited so declarations are not compared one by one.

An `Analysis` is cached until a pass fails to preserve it. A pass returns `PreservedAnalyses`: `all()`, `none()` or `function()` (every analysis that reads only the function), adjusted with `preserve::<A>()` and `except::<A>()`. An entry that reads nothing outside its function (`READS_OUTER = false`) outlives a change to the module. `LLRM_CHECK_REPLAY` asserts that an `update` or `unaffected` answer equals a fresh one; `LLRM_CHECK_MODULES` does the same for a kept module result. How module analyses are brought up to date after an edit: [module-analyses](module-analyses.md). When the summaries are made: [early-summaries](early-summaries.md).

## Control flow and SSA

| Module | Owns | Consumers |
|---|---|---|
| `cfg` | Blocks as `graph` walks them (block id, layout order). `Shape` is dominance, natural loops and a lazily built reaching-block closure. | nearly every pass; backend `isel` |
| `graph/loops` | Which blocks are a loop and how deep each nests, for any IR whose blocks are `loops::Node`s. Shared with the machine phases. | `cfg::Shape`, loop passes, backend `allocate`, `intervals`, `splitkit`, `spillplacement` |
| `ssa` | `use_index`, substitution of a value by an operand, `pruned_phis`, and `SsaUpdater`, which places phis where a value's definitions meet. | `promote`, `gvn`, `rotate`, `jumpthread`, `lcssamerges`, `loadjoins`, `dead`, `floatloop` |
| `occurrence` | Instruction and phi walks keyed by `InstId`, so a key outlives a rewrite. | `loopexit`, `unswitch`, `induction` |
| `liveness` | Which values are live where, and register pressure (`pressure`, `live_points`). | `spill`, `homes`, `lsr`, `indvars`, `inline` |

`cfg::Shape` is built once per function and carried by `Unit::shape`. The backend has its own liveness over physical register lanes.

## Known values and ranges

`consts` is the base: every other fact in this section reads it.

| Module | Owns | How | Consumers |
|---|---|---|---|
| `consts` | Which values are known numbers. A fact is a width and a number: the low `width` bits are `n`. | [`known`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/consts.rs#L820) iterates the function's values to a fixed point; `known_walked` also solves memory cells when the caller says what each call writes (`Calls`). | `fold`, `algebraic`, `gvn`, `hoist`, `indvars`, `loopexit`, `loopmotion`, `inline`, `counting`, `unswitch`, `profit`, `ipacp`, `interprocedural`, backend `isel` |
| `constant_cycles` | Constants across loops and phis, along the edges that run. | Sparse propagation with distinct pending and overdefined states and an optional successor evaluator. An unresolved branch keeps every successor. | `decide`, `consts` |
| `ranges` | Non-wrapping integer intervals, scoped to the branch edges and counted loops that bound them. | `dominated_edges` solves the unavoidable branch edges per block; `bounded` adds what each counted loop gives its blocks; `exact_offsets` finds offsets known from the start of their object. | `decide`, `algebraic`, `fixednarrow`, `hoist`, `loopexit`, `ports`, backend `isel` |
| `guards` | What the branches dominating a block prove on entry to it, by direct implication of the same two sides. | `guards` climbs at most 64 dominators; `on_edge` adds the branch's own edge; `holds` and `decides` answer a compare. | `jumpthread`, `exitfold`, `decide` |
| `difference` | What a set of compares proves of the unknowns they test: LLVM's ConstraintElimination. | Each compare is `x - y <= c`; [`proves`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/difference.rs#L59) closes the set. An unsigned compare is treated as signed where both sides are known non-negative. | `guards` |
| `assumptions` | The conditions of `llvm.assume` calls that hold at a block. | Found once per body, then read by the blocks above in the dominator tree. | `guards`, `ranges` |
| `demanded` | Which bits of each integer value anything reads: LLVM's DemandedBits. | Backward from the users; a user outside `valuetracking::demanded_operands` reads every bit; loop phis iterate to a fixed point. | `algebraic` |
| `floatfacts` | Exact finite values of floating operations. | A `Rule` per instruction; a fact is exact in its format, so it holds under any rounding mode. Floats, integers and memory are solved in one pass (`FloatFacts`). | `floatfold`, `floatloop`, `fold` |
| `floatbounds` | Floating operations that are exact over bounded, not known, integers. | Interval arithmetic over `floatfacts` rules; a load from an array indexed by a counted loop reads at most 64 elements. No pass calls it. | none |
| `parameter_ranges` | The range of each integer parameter of a body only the program calls. | Callers' actuals are read with `ranges`, joined with the last round, widened after the second; a range still growing after `ROUNDS` is withdrawn. | `interprocedural` |

`Registers` is `consts::known` with no calls; `ThroughMemory` is `known` with memory solved and each call writing what `CallEffects` says. `ranges` is the one a pass reads when it needs a bound, not a value. What `consts` and `ranges` feed: [constant-propagation](constant-propagation.md).

## Loops

| Module | Owns | How | Consumers |
|---|---|---|---|
| `induction` | Which values are affine in a loop's counter, and how many trips a loop makes: LLVM's ScalarEvolution add recurrences and trip counts. `Recurrence` is `pointer + start + step * trip` over invariant symbols (`Scev`); `users` is what reads them. | [`counted_all`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/induction.rs#L530) proves each loop's compare, step and exit; `trip_count` and `exits` give the count. | `indvars`, `loopexit`, `exitfold`, `lsr`, `rotate`, `peel`, `unroll`, `unroll_jam`, `loopmotion`, `hoist`, `window`, `mirror`, `fill`, `expand`, `decide`, `counting`, `floatloop`, `algebraic`, `profit`; backend `isel` |
| `peelsize` | Whether a loop is worth copying out completely. | GCC's `try_unroll_loop_completely` decision with LLVM's size: each iteration is run over the values it knows, folded work is free, and only the successors a folded branch leaves are followed. Budgets are MIR instruction counts. | `peel`, `unroll`, `pipeline`, `profit` |
| `dependence` | Data dependence distances over a two-deep nest, and the outer unroll factor unroll-and-jam may use. | Accesses are `constant + outer * i + inner * j` byte offsets into one object; pairs that differ in coefficients are `Unknown` and refuse a jam where one writes. [`factor`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/dependence.rs#L134) follows gcc's `adjust_unroll_factor`. | `unroll_jam` |
| `loopclone` | Clones of a loop's iterations as a CFG, keeping a residual loop and every exit. Not a profitability decision. | [`peeled`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/loopclone.rs#L102) returns a changed copy of the function. Live-outs must be in LCSSA. | `peel`, `unswitch`, `unroll_jam` |

`Counted` and `Followers` are the manager entries over `induction`; both read `Registers`. The passes: [unrolling](unrolling.md), [lsr](lsr.md).

## Memory

The memory analyses share one vocabulary, in `memory`. See [strong alias analysis](../architecture/strong-alias.md) for the object and slice model.

| Module | Owns | How | Consumers |
|---|---|---|---|
| `memory` | Object kinds (`Frame`, `Global`, `Parameter`, `Unknown`, `Nonlocal`, `Absolute`), byte `Slice`s with stride, `Provenance`, `MemRef`, and the [`Unit`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/memory.rs#L715) every analysis reads. | `Unit` holds the function, module facts and optional manager results; with a field unset, each ask finds the fact itself. | `gvn`, `dse`, `promote`, `hoist`, `loopmotion`, `fold`, and most others; backend `isel` |
| `regions` | Whether two references can meet: `may_alias`, `overlapping`, `typed_apart` (TBAA), and the `OverlapBucket`s a `CellMap` indexes by. | Compares objects, slices, displacement off one base and access types. Every MemorySSA consumer reaches it through `overlapping`. | `promote`, `homes`, `loopmotion`, `fill`, `floatloop`; `memoryssa`, `avail` |
| `cellmap` | Facts keyed by memory cell, indexed by the object each cell lies in. | The caller hands each new cell's bucket and span to `insert`. | `promote`; `alias`, `avail`, `consts` |
| `sliceset` | A set of slices that holds the module's callbacks as a shared run. | A set is a `Shared` run by reference plus the few slices of its own; as a set it is the exact union. | `alias` summaries; `interprocedural` |
| `alias` | Flow-sensitive pointer provenance (`points_to`), per-function `Summary`s of reads, writes and captures, and each call's effect (`calls_annotated`). | Facts flow through SSA phis, `select`s and exact pointer spill slots; unknown stores kill spill facts and never prove disjointness. `summaries` iterates callee summaries to a fixed point. | `gvn`, `loadjoins`, `decide`, `promote`, `interprocedural` |
| `pointerfacts` | Relative byte offsets of pointers reached from one base by `getelementptr inbounds` with constant indices. | `inbounds` is the proof that both pointers lie in one allocation. | `argpromotion`; `memoryssa` |
| `frameescape` | Which frame objects' addresses reach where (`exposed_allocas`, `framed`). | An alloca is exposed when a use of it is not a move, an access through it, or a marker. | `ExposedFrames`; backend `isel` |
| `globalsaa` | Which globals no code outside the program reaches but by name. | A global is tracked when no module lets its address escape and it is not exported, or the runtime module lists it in `!llrm.named`. | `alias`, `globaldce`, `globalopt` |
| `memoryssa` | One memory state: stores and writing calls define it, loads and reading calls use it. A clobber walker skips writes `may_clobber` rules out. | [`built`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/memoryssa.rs#L1285) wires block entries first, then eliminates identity phis. `Accesses` says what each instruction touches. Rebuilt after a function changes. | `gvn`, `dse`, `promote`, `hoist`, `loopmotion`, `loadjoins`, `homes`, `spares` |
| `avail` | Loads a known value serves, and stores that are dead. | `forwardable_walk` finds each load's value by a MemorySSA walk, serving a read only with a value of its own type; `loadjoins` forms a phi where predecessors differ. `dead_stores` runs backward: a cell is dead when everything after overwrites it before reading. | `loadjoins`, `dse`, `transform`, `consts` |
| `nearest` | The items placed in blocks that dominate a point, nearest first. | Blocks per key sorted by dominator-tree preorder, each linked to the nearest holder above it. | `loadjoins` |
| `observers` | Which cells nothing outside the function can read. | A frame object whose address did not escape (`alias::points_to`) and that no access reaches unnamed. | `dse` |
| `effects` | Whether a call or its callee states a fact (`readnone`, `memory(...)`, `noreturn`, ...). | `states` reads call-site and callee attributes; `touches_memory`, `writes_memory`, `unmodeled_write`. | `interprocedural`, `noreturn`, `branchprob`, `profit` |

The manager entries over `alias` are `Pointers`, `PointerValues` (the pointer solve before escape, shared by `Pointers` and `CallEffects`), `Annotated` (each access's `MemRef`) and `CallEffects`. `Summaries` uses [`summaries_updating`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/alias.rs#L1080): when only some bodies changed, the dirty bodies and every body that reads them through a chain of calls are solved again and the rest is kept. `ProgramSummaries` solves one fixed point across the program's modules. Design notes: [call effects as a summary queried per call](../architecture/modref.md), [points-to kept across edits](../architecture/points-to-lifetime.md). What gvn does with `memoryssa` and `avail`: [gvn](gvn.md).

## Calls and control

| Module | Owns | How | Consumers |
|---|---|---|---|
| `noreturn` | Which bodies and blocks cannot reach a normal return, and which blocks are cold. | `terminal_sites` joins a call's and its callee's `noreturn`; `cold` is a least fixed point of blocks all of whose paths end in a terminal call, `unreachable` or a `cold` call. | `hoist` (`TerminalSites`), `interprocedural`, `branchprob`, `induction` |
| `branchprob` | Static branch probabilities and block frequencies. | First applicable heuristic in LLVM's order (all-`unreachable`/cold successor, loops, counted-loop trips, ...) then GCC's call and return heuristics; `propagated` turns edge odds into frequencies. | `profit`, `rotate`; backend `isel`, `jumps`, `analysis/frequency` |
| `interprocedural` | Facts that cross direct calls: constant arguments and returns, purity, erasable calls, noreturn procedures. | Computed over the module (or the program: `program_returns`, `program_parameters`) and materialised at the call. A call stays unless a purity proof says its effects are unobservable. | `interprocedural` pass, `ipacp`, `inline` |

`interprocedural` consumes `effects`, `noreturn`, `frameescape`, `pointerfacts`, `sliceset` and `parameter_ranges`.

## Test support

`generated` (random loops for checking recurrence forms against the interpreter; `SCEV_SEED`, `SCEV_SEEDS`) and `testing` build only under `cfg(test)` or the `testing` feature.
