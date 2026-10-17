# The editing contract: one edit record, and facts that declare what they read

TL;DR: every incremental idea that depended on the editing substrate failed on today's one (preserved sets kept 9%, `Annotated` per entry found 74.7% invalid, an in-place spill gained nothing, fact placement saved 9% for 23% of objects changed). This page fixes the substrate once, so that the builders do not each invent a record: a body version identifies content; an edit is a primitive that logs one `Change`; a fact declares what it read, body and non-body, and is kept, updated or dropped from the log; a pass never names what it preserves; a pass asks per-query facts at the site with a cap and reads held solves; speculation is checkpoint and rollback. The design is checked in isolation first, by a trace-and-replay harness (section 4), with `gvn` as the first and calibration target.

## 1. What the contract answers

| question | measured |
|---|---|
| how often is a fact recomputed for a body version it already has | 545 of 2,742 function-level computes (20%, 41.3 M, 1.4% of the `d_faces` -O2 compile) |
| how often has a consumer no log to read | `changes_since` is `None` (lineage replaced by a fork or a rollback) for 18.1% of the weight of 93 `MemorySSA` structure builds (hoist-bounded) |
| how often does an edit set change the CFG | 41.9% of that weight (terminator rewritten or moved across blocks, blocks made or erased: `jumpthread` and simplify batches); the other sets only erase (27) or rewrite (53) memory operations |
| where do the frequent passes pay | the tower under `fold`, `decide`, `promote`, `sroa`, `hoist`: 403 M own (12.8%); `decide` asks at 574 branch visits and decides none |
| what a cap buys | `fold` with a walk of at most 6 steps per load keeps all 968 of its edits and `d_faces` -O2 falls 2.4% (mem ladder N=1600: -20.7% of the compile, `fold` linear); 11 of 803 corpus objects differ at 6, 4 at 20, none at 100 |
| what placement buys | bounded solved only at `fold` and `algebraic`: -2.9% of `d_faces` -O2, 3 of 803 objects differ, rows: one row (`x_transpose`) +1.0% to +1.4% clocks, geomean unchanged; at `fold` alone `tile` is +36.7% clocks and the rows rule kills it |

## 2. LLVM structure, ours, and which we mirror

**Analysis invalidation.** LLVM: a pass returns `PreservedAnalyses`; the manager asks each held result to `invalidate(IR, PA, Inv)` and an `Invalidator` walks the dependencies between results ([`PassManagerImpl.h#L169-L196`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/PassManagerImpl.h#L169-L196)):

```cpp
inline void AnalysisManager<IRUnitT, ExtraArgTs...>::invalidate(
    IRUnitT &IR, const PreservedAnalyses &PA) {
  // We're done if all analyses on this IR unit are preserved.
  if (PA.allAnalysesInSetPreserved<AllAnalysesOn<IRUnitT>>())
    return;

  // Track whether each analysis's result is invalidated in
  // IsResultInvalidated.
  SmallDenseMap<AnalysisKey *, bool, 8> IsResultInvalidated;
  Invalidator Inv(IsResultInvalidated, AnalysisResults);
  auto ResultsListI = AnalysisResultLists.find(&IR);
  if (ResultsListI == AnalysisResultLists.end())
    return;
  AnalysisResultListT &ResultsList = ResultsListI->second;
  for (auto &AnalysisResultPair : ResultsList) {
    // This is basically the same thing as Invalidator::invalidate, but we
    // can't call it here because we're operating on the type-erased result.
    // Moreover if we instead called invalidate() directly, it would do an
    // unnecessary look up in ResultsList.
    AnalysisKey *ID = AnalysisResultPair.first;
    auto &Result = *AnalysisResultPair.second;

    auto IMapI = IsResultInvalidated.find(ID);
    if (IMapI != IsResultInvalidated.end())
      // This result was already handled via the Invalidator.
      continue;

    // Try to invalidate the result, giving it the Invalidator so it can
```

Ours: the manager keeps, updates or drops a result by its mark and the change log (`Analyses::get`); the pass names nothing. Mirror the dependency walk and the per-result decision (`unaffected`, `update`); deviate on who declares: LLVM's pass declares what it preserved, ours derives it from the log, which is exact where a declaration is a promise (jumpthread's `none()` below is a declaration that costs 101.5 M). Reason: the walk costs the same in both (2.85 M against 2.63 M, [analysis-manager](../comparison/llvm/analysis-manager.md)), which says nothing about keeps; the number that decides is the keep rate of log-derived keeps against a declared-set baseline, which the replay harness (section 4) must report before a pass drops its declaration.

**Edits and undo.** LLVM: IR edits are O(1) on an intrusive list with use chains; undo is SandboxIR's `Tracker`, one `IRChangeBase` per edit with `revert` and `accept` ([`Tracker.h#L127-L139`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/SandboxIR/Tracker.h#L127-L139)):

```cpp
class IRChangeBase {
protected:
  friend class Tracker; // For Parent.

public:
  /// This runs when changes get reverted.
  virtual void revert(Tracker &Tracker) = 0;
  /// This runs when changes get accepted.
  virtual void accept() = 0;
  virtual ~IRChangeBase() = default;
#ifndef NDEBUG
  virtual void dump(raw_ostream &OS) const = 0;
  LLVM_DUMP_METHOD virtual void dump() const = 0;
```

Ours: `Function` edits go through primitives that log one `Change` ([`module.rs#L136-L174`](https://github.com/ali-mosavian/llrm/blob/6ce94db4be8d04ca2def6a5eac8bd220ba58a8d1/crates/ir/llrm-mir/src/module.rs#L136-L174)):

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Change {
    Inserted {
        inst: InstId,
        block: BlockId,
        next: Option<InstId>,
    },
    Cloned {
        from: InstId,
        to: InstId,
    },
    Moved {
        inst: InstId,
        block: BlockId,
        next: Option<InstId>,
        from: BlockId,
    },
    Rewritten(InstId),
    /// Every use of `value` is about to become an operand that is the same
    /// value (`replace_value`: a common subexpression, a load's stored value, a
    /// constant folded to), each as an `Operand` entry then a `Rewritten`
    /// one. What an analysis said of any value stays true.
    Substituted {
        from: ValueId,
        to: Operand,
    },
    /// Operand `index` of `inst` was `was`: logged before the `Rewritten` of
    /// the change, by what reads what an instruction read before (the
    /// liveness of a value it stopped reading, the edge a branch stopped
    /// taking).
    Operand {
        inst: InstId,
        index: u32,
        was: Operand,
    },
    /// The operands of `inst` were `was`, before the `Rewritten` that follows.
    Operands {
        inst: InstId,
        was: Box<[Operand]>,
```

with `Mark`, `changes_since` and the `Touched` digest over a window, and (draft #1945) `checkpoint`, `rollback`, `rollback_keeping`, `redo`, `release` over the same log. Mirror: the Tracker's shape, an append-only record of reversible edits with save, revert and accept; the primitives' O(1) is the S6 result (`erase_all`, `move_all` are the range splice; the intrusive order itself only if a slope remains).

**Fact interface.** LLVM: required and preserved sets per pass (`getAnalysisUsage`), and per-query analyses with caps. Ours ([`passes.rs#L264-L299`](https://github.com/ali-mosavian/llrm/blob/6ce94db4be8d04ca2def6a5eac8bd220ba58a8d1/crates/ir/llrm-mir/src/passes.rs#L264-L299)):

```rust
pub trait Analysis: 'static {
    type Result: PartialEq + std::fmt::Debug + 'static;
    const NAME: &'static str;
    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result;

    /// Whether the result reads `analyses.outer()`, itself or through what
    /// it builds on. One that does not outlives a change to the module.
    const READS_OUTER: bool = true;

    /// Whether `update` can bring a result the function has since changed up to
    /// date: such a result is kept past an invalidation, with the point in
    /// the function's history it was true at.
    const INCREMENTAL: bool = false;

    /// Whether `unaffected` can tell a result still true after `changes`: such
    /// a result is kept past an invalidation, and stands again when
    /// `changes` leave it as it was (and, if it reads the outer facts, those
    /// are the ones it read).
    const SKIPS: bool = false;

    /// Whether the result stays a true answer, with entries for fewer values
    /// and facts no more precise than a fresh derivation's, after edits that
    /// `sound_edits` finds only keep every live value's meaning: what it says
    /// of a live value holds, a value it has no entry for is unknown, and none
    /// is asked of an erased one. `covers` is the check.
    const SOUND: bool = false;

    /// Whether the result holds values (a pointer, a root), not facts keyed by
    /// them: uses replaced by an equal value (`Substituted`) then leave it
    /// naming one that is gone, and it is not kept past them.
    const NAMES_VALUES: bool = false;
```

Mirror the caps and the required set; deviate on the declaration site, as above.

## 3. The contract

**3.1 A body version identifies content.** `Mark` is a point in one lineage's history; a lineage is a function and its edits, a fork starts its own. A version equal in two functions means equal content, forks included: two gvn forks edited apart are two versions (the bug fixed in #2047 was one point for both). Adopting a fork replaces the function's lineage; discarding it drops its log. `changes_since(mark)` has a defined answer when the lineage was replaced: `None`, and a consumer then treats the body as new (18.1% of the weight in the probe, so the path must be cheap: rebuild, not panic). A `Mark` is (lineage, epoch, position): a rollback moves the position back and starts a new epoch, so equal marks mean equal content. A fact computed in the rolled-back span is dropped; the result held at the checkpoint is restored, not rebuilt, so pricing by checkpoint and rollback costs no recompute. Forks that remain after rollback replaces them (none planned: 144 clones today, `stamped` 84, gvn 38) are the only users of the `None` path.

**3.2 An edit is a primitive that logs.** Every change to a body goes through `Function`'s primitives (`insert`, `erase`, `erase_all`, `move_to`, `move_all`, `set_operand(s)`, `replace_value`, `create_block`, `insert_block`, `erase_block`, `clone_instruction`), each logging exactly the records above, in order. There is no bulk write. A pass that rebuilds blocks builds them with the primitives; `jumpthread` does (`create_block`, `insert_block`, `insert`, `clone_instruction`, `set_operands`, `erase`), so its edits are in the log and a counter that reads zero for it is the counter's fault (it counted a pass the pipeline's step did not wrap). Its end, which names a preserved set by invalidating everything and re-running four passes ([`jumpthread.rs#L226-L249`](https://github.com/ali-mosavian/llrm/blob/6ce94db4be8d04ca2def6a5eac8bd220ba58a8d1/crates/opt/llrm-transforms/src/jumpthread.rs#L226-L249)):

```rust
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if !threaded(unit.context, unit.layout, unit.function, &outer, self.size, self.correlated, self.states) {
            return PreservedAnalyses::all();
        }
        // What the copies know of their state: a product by a constant, a
        // compare of one. Only where something was copied.
        analyses.invalidate(unit.function, &PreservedAnalyses::none());
        for pass in [
            &mut crate::fold::Fold as &mut dyn FunctionPass,
            &mut crate::algebraic::Algebraic { size: false },
            &mut crate::decide::Decide,
            &mut crate::dead::Dead,
        ] {
            let kept = pass.run(unit, analyses);
            analyses.invalidate(unit.function, &kept);
        }
        PreservedAnalyses::none()
    }
}

/// One path found: the block its first edge leaves, the blocks copied, and the
```

What a reader needs per kind is in the record or in the digest, not in a second log: memory operations added, removed, moved or rewritten (`Touched.memory_*`); a pointer operand edited against another operand (`Touched.pointers`, from the `Operand`/`Operands` records and the types); terminator and CFG edits (`terminators_made`/`rewritten`, `blocks_created`/`erased`, `cfg`); parameters (`Parameters`); substitutions (`Substituted`: meaning kept).

**3.3 A fact declares what it read, and nothing else keeps it.** Each fact states: (a) the parts of the body it reads, as the digest's bits (`Depends`: models, keyed, flows); (b) every input that is not the body, each with its own version: the outer parts (declarations, callee effects, summaries, globals), the exposed-allocas set, the shape's back edges, the globals view (`captured` per global: the same body solved with and without it differs in 12 of 113 repeated `PointerValues` solves); a fact that declares no version for an input cannot be shared or kept; (c) how it follows an edit: `unaffected(changes)` (kept as it is), `update(previous, changes)` (equal to a fresh run, asserted by `LLRM_CHECK_REPLAY`), or neither (dropped). Facts never define identity (A2): an interned object or cell has an identity independent of whether a solve ran, so that a kept or skipped computation changes no id, spelling or order (`LLRM_CHECK_INTERN`).

**3.4 A pass names no preserved set.** A pass edits through the primitives and returns; the manager derives, from the log, what each held fact needs. `analyses.invalidate(.., none())` in a pass is a defect to be replaced by the log (`fold` after `_numbers`; `jumpthread` after threading). The saving is bounded by the facts' share of jumpthread's 101.5 M, which is four passes re-run; keeping the facts does not remove their own work.

**3.5 Facts asked mid-pass.** Two kinds. A per-query fact (known bits to a depth, one access's `MemRef`, escape-before, one load's value) is asked at the site that needs it, with a cap that is a parameter of the consumer, as in LLVM: `fold` 6 steps (`FindAvailableLoadedValue`, `MaxInstrsToScan`), the others 100 (`memdep-block-scan-limit`, `memssa-check-limit`), under a shared safety bound. A whole-function solve (constants through memory, loop bounds, float facts, provenance) stays per body version and is kept by 3.3; placing one (run at the passes whose edits consume it, read held-or-conservative elsewhere) is allowed only for a family whose rows pass (bounded at `fold` and `algebraic` did; blanket placement was killed). LLVM's mirror is facts on the IR (redesign-plan 3.1(c)): a pass that solves writes the result into the IR and later passes read the IR, not a held lattice. Why not ask lazily for the solves: [fact placement](fact-placement.md), section "The rule". A withheld result is marked and skipped by the replay oracle, which would otherwise call an empty `Bounds` stale.

**3.6 Undo.** Speculation (hoist's and gvn's pricing, a candidate rewrite) is `checkpoint` and `rollback` on the live function; the journal is the change log. There are no clones in the pipeline (144 at -O2 today: `stamped` 84, gvn fork 38, `sink` 11, interprocedural 11). The cost side is measured (about 0.1% of the compile); the benefit is an enabler.

**3.7 The log's own cost, and CFG updates.** The log is truncated to the oldest mark any held fact or checkpoint holds; with none held it is empty. Its growth and the `Operand` records per use on `replace_value` are measured by the recorder before a pass depends on it. A CFG-changing set (41.9% of the structure builds) is today a rebuild; the mirror is LLVM's `DomTreeUpdater` and `MemorySSAUpdater::applyUpdates`: an edge delta derived from the terminators' `Operand` records, dominators first. The trait consts (`SOUND`, `SKIPS`, `INCREMENTAL`, `NAMES_VALUES`) are the implementation of 3.3(c) and retire as `depends` plus `update` replace them.

## 4. The trace-and-replay harness

**Why.** The structural changes are checked in isolation before any pass is rewritten: a component today against the component as it would be, outside the pipeline, counting the engine's instructions only. Every isolated result is a go or no-go filter; the vertical slice in the real pipeline decides.

**Recorder.** From real compiles (`d_faces` -O1/-O2/-Os, the bench loops, the joins, unrolled and mem ladders) an event stream per function: pass start and end; each edit with its kind (erase, rewrite, insert, move, CFG) and the instruction or block; each fact ask with its consumer and key and what the manager did (hit, update, recompute); the body size at each event. The edit and ask events come from the primitives and `Analyses::get`; the pass events from the pipeline step.

**Replay.** A standalone spike crate (no pipeline) feeds the stream to an engine variant and counts its instructions: today's engine (recompute per version), the candidate (per-edit update, derived preserved sets, per-query facts with caps). Two levels: an event replay for policies that need only kinds and sizes (how many recomputes and updates a policy performs), and a dump replay for real work (the MIR dumps at a pass's input, `tools/stages.py`, run through the pass with each variant).

**Targets, in order.** (a) `gvn`: today's pass against `gvn` with `erase_all`, facts kept or updated per edit, per-query facts and capped walks, on the dumps at its input; the contract and edit API are provided here, the variants are greedy-port's; `gvn` is the calibration pass. (b) The fact engine for `ThroughMemory`, `Accesses`, `Pressure`, `Counted`: recompute per version against update per edit. (c) Body storage: `Vec` per block against an arena with the change log (S1, S6). (d) The allocator's round: `LiveRanges::edit` against an edit-record update (S4's in-place idea, killed in the pipeline); replayed as a second calibration, where the result must agree with the pipeline's.

**Limits, stated.** Event replay is valid only for output-identical policies: a cap or a placement changes the answers, hence the later asks and edits, so those go through the dump replay. A trace records today's edit shapes (big batches, whole-body rebuilds), so replay gives an upper bound for today's passes; synthetic traces with LLVM-shaped edits (one edit, one ask) show what the structure buys if the passes change; cache and working-set effects and the integration cost are not captured. **Status:** the recorder is `LLRM_TRACE` (#2076); `tools/replay/keep.py` is the first event replay. Of the recomputed asks (exclusive instructions, `d_faces` -O2, 1,782 asks): 15% first computes of a body or lineage, 10% rebuilt for an unchanged version, 74% after an edit a fact reads, 1% after edits none of its reads include. A log-derived keep with the declared-read model therefore saves at most 11% of the fact work there (about 58 M, 2% of the compile; the 114-compile corpus: 16%); the rest needs `update`, not a keep. The model is conservative (an analysis outside the CFG and memory groups counts any edit as read), so these are lower bounds for keeps. The doubt that the 74% is inflated by "any edit is read" does not hold: the three costliest analyses (`through-memory`, `pointer-values`, `annotated`) were already on the memory-reads set, and moving `call-effects` there too leaves it at 74.3%. The trace carries digest bits, not which operand fed a store, so a finer read set cannot be applied from it. By analysis, over the corpus, exclusive M recomputed after an edit the fact reads (and whether it has an `update` today): `bounded` 908 (yes), `through-memory` 636 (no), `annotated` 350 (no), `pointer-values` 331 (no), `call-effects` 306 (no), `counted` 103 (yes), `registers` 46 (yes); 3,611 in all. Facts with no `update` are 45% of it (1,623 M); `bounded` and `counted` have one and still recompute 1,011 M, so what an `update` buys is limited by how often it is taken. `tools/replay/bounded.py` splits `bounded`'s recomputes by whether the edits since its last result changed the CFG: 903 of 908 M (99%) follow a CFG-changing batch, where `update` is refused; and its updates cost 691 K per ask against 915 K for a recompute (`d_faces` -O2: 0.85 M against 1.0 M), so taking them buys little. The fix is the CFG part (3.7: edge deltas) and diffing loop structure, not another updater. `counted` is taken even after CFG edits (1,333 of 1,552 CFG-edit asks). `tools/replay/price.py` prices every family at once (`d_faces` -O2, exclusive M; today's recompute work is 542 M, 21% of the compile): the work after an edit the fact reads is 74% of it; an updater at `counted`'s measured ratio (0.33 of a recompute) leaves 214 M (8.3%, saves 12.8% of the compile), at `bounded`'s (0.75) leaves 383 M (saves 6.2%); a capped per-query ask for the memory families (residual 0.17, from the cap-6 result on `fold`'s walk) with the updater for the rest leaves 165 M (saves 14.7%). Free keeps and first computes are priced in. Bounds, not predictions: the updater's own touch cost and the cap's output changes (11 of 803 objects at 6) are not charged, and the 0.17 is measured on one consumer. Calibration against the pipeline's own counts: 598 same-version rebuilds against 545 measured by the manager (a different count of nested asks).

## 5. Decisions for the sessions that build on it

| fact or component | owner | reads (declared) | non-body inputs (each versioned) | follows an edit by | must use from the contract |
|---|---|---|---|---|---|
| `MemorySSA` structure and walker | hoist-bounded | memory operations; terminators and blocks | alias answers (`Annotated`), call effects | log records; a CFG-changing set rebuilds (41.9% of the weight) until a split-block case is handled; an updater API (`insertDef`, `removeMemoryAccess`, LLVM's `MemorySSAUpdater`) the pass may call as a fast path with the log as the fallback; no `MemorySSA`-special log | `changes_since`, `Touched.memory_*`, `cfg`; `None` means rebuild |
| `Annotated`, `PointerValues` | keep-analyses | pointer operands, memory operations (`Touched.pointers`) | exposed allocas, the globals view (`captured`), callee attributes | per entry, from `Operand`, `Operands` and `Substituted` records; an add or compare operand edit does not touch them | the declared-read versions; identity independent of facts |
| `Pressure`, spill forecast | greedy-port, hoist-bounded | value definitions and uses per block | target room, frequencies | `Moved`, `Erased`, `Inserted` of a candidate; speculation by `checkpoint`/`rollback`, no clone | rollback; the same record shape |
| `LiveRanges`, spill (LIR) | ssa-spill-m16 | instructions and operands of the LIR body | register classes, slots | its own `Change`/`Touched`/`Mark` of the same shape, plus slot numbers with gaps; a fresh value's occurrences are in its `Inserted` records | the record and digest shape, not a second one |
| `Counted`, `Bounded`, SCEV-like loop facts | edit-contract-replay | the loop's header phi, step, compare and bound operands; the loop's blocks | the registers' facts, the shape | unaffected unless an edit is in the loop's blocks or those operands; else per loop. `Bounded` recomputes after CFG-changing batches (99% of its recompute in the corpus), where `update` is refused: it takes the CFG edge deltas (3.7) and loop-structure diffing, with `Counted` (update taken after CFG edits) as the model; ceiling 3.5% of `d_faces` -O2, behind the cap and `decide` work | `Touched.blocks`, `operands_changed` |
| `Registers` | edit-rollback | the values' definitions | none beyond the body | `SOUND` plus `INCREMENTAL` today; `Standing` in `indvars` to use the manager's `update` instead of re-deriving it (26 M, 0.9%) | `update` |
| per-value congruences | keep-analyses | on demand from the body | none | not kept: computed on demand, no memo | none |

## 6. What is not known

- The cost of a capped per-load walk for `ThroughMemory` as `facts.at(load)` against the whole build, per ask (the cap inside the existing build is measured above; the per-load form is not).
- Whether a withheld empty fact is worse than the conservative one for a consumer that assumes a fact exists (the placement probe's 15-23% differing objects).
- The rows of the cap at 6 and 20 (queued) and of placement at other positions.
- The alias tower (`Annotated`, `Pointers`) under placement.
