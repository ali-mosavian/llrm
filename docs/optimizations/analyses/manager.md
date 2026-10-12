> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/manager.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/manager.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/manager.rs sha256:d16aaf2e1a74db88f8fa603de83920fd69a1a437bd25e3956212d7cd19e842cb -->

# manager

The memory and range analyses as entries of llrm-mir's analysis manager:
asked for by name, cached, kept or brought up to date after a pass.

## How it works
A pass asks `analyses.get::<A>(context, layout, function)` and receives a
shared result. The manager computes it once, hands the same `Rc` to every
later asker, and decides after each pass what to do with it. The entries
here declare how, through the `Analysis` trait of `llrm_mir::passes`:

- A pass returns the `PreservedAnalyses` it left true. An entry not named is
  dropped, unless `READS_OUTER` is false (it reads only the function, as
  `DemandedBits` and `Shape`) and the function is unchanged.
- `SKIPS` with `depends`: the entry is kept past an invalidation if the
  change log cannot have altered it. `Depends` names the instructions the
  result models, those it keys entries by, and the users a change flows
  through. `ExposedFrames`, `PointerValues`, `Pointers`, `CallEffects` and
  `Writes` model pointers, aggregates and calls only, so a change to integer
  arithmetic leaves them standing.
- `INCREMENTAL` with `update`: the entry is brought up to date from the
  change log. `DominatedEdges`, `Bounded` and `Counted` redo only the blocks
  or loops `reached_by` the changes (a change to a terminator or to the CFG
  redoes all); `Accesses` in [`crate::memoryssa`](memoryssa.md) survives pure moves.
- `LLRM_CHECK_REPLAY` asserts that every kept or updated result equals a
  fresh `run`.

```text
 pass runs ---> PreservedAnalyses
                  |
   entry named? --yes--> kept
        |no
   READS_OUTER=false and function unchanged? --yes--> kept
        |no
   SKIPS and depends.unaffected(changes)? ------yes--> kept, stands again
        |no                                              if changes undone
   INCREMENTAL and update(previous, changes)? --Some--> updated
        |no
   dropped; recomputed on the next get
```

Function entries and what each asks for (every one also asks `Shape`, and
most `AssumptionCache` and `ExposedFrames`, which are omitted):

```text
 PointerValues    -
 Pointers         PointerValues
 CallEffects      PointerValues
 Writes           CallEffects
 Registers        -
 Followers        Registers
 Counted          Registers
 DominatedEdges   Registers
 Bounded          Registers, Counted, DominatedEdges
 Annotated        Pointers, Registers, Counted, DominatedEdges, Bounded
 Accesses         Annotated, CallEffects                  (memoryssa)
 ThroughMemory    Writes, Annotated, Registers
 FloatFacts       ThroughMemory, Writes, Annotated, Registers
 Published        Pointers, ExposedFrames                 (observers)
```

Module entries read the program through `ModuleAnalyses` and are dropped
unless every run of a pass preserved them. `GlobalsAA` (which globals no
outside code reaches but by name) and `Summaries` (each defined function's
memory effects, by name) can be held past passes that add no memory
operation when `covers` says the stale result states no more than a fresh
one. `Summaries` keeps per-body facts and recomputes only bodies whose
history moved. `Callbacks`, the effects of all the callbacks into the module
added up, is rebuilt only when `GlobalsAA` or `Summaries` is a different
result. `ProgramSummaries` is a program entry: one fixed point over all
modules, each module's `Summaries` given the others'.

`Unit::within` builds the `Unit` that analyses read from `Outer` (the
module, target and program), carrying whichever cached results the caller
holds. `Held` and `Ranges` bundle the cached results a pass passes on to a
`Unit`, so the facts are the manager's and not a solve of the pass's own.
`Bounded` is empty where `Outer::loop_intervals` is off (-O1).

## Provides
The entries: `ExposedFrames`, `PointerValues`, `Pointers`, `Annotated`,
`CallEffects`, `Writes`, `Registers`, `DemandedBits`, `AssumptionCache`,
`Followers`, `Counted`, `ThroughMemory`, `FloatFacts`, `DominatedEdges`,
`Bounded`; module entries `GlobalsAA`, `Summaries`, `Callbacks`; program
entry `ProgramSummaries`; `Held`, `Ranges`, `reached_by`, `call_effects`,
`writes`. [`crate::cfg`](cfg.md)'s `Shape`, [`crate::memoryssa`](memoryssa.md)'s `Accesses`,
[`crate::observers`](observers.md)'s `Published` and [`crate::noreturn`](noreturn.md)'s `TerminalSites`
are entries defined beside their analyses.

## Needs
[`crate::alias`](alias.md), [`crate::consts`](consts.md), [`crate::globalsaa`](globalsaa.md),
[`crate::induction`](induction.md), [`crate::ranges`](ranges.md), [`crate::floatfacts`](floatfacts.md),
[`crate::frameescape`](frameescape.md).

## See also
LLVM's new pass manager `AnalysisManager` and `PreservedAnalyses`; the trait
is in `crates/ir/llrm-mir/src/passes.rs`.
