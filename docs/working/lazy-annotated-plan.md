# Lazy Annotated and the MemorySSA cost: plan

Owner: funcattrs-port. Written 2026-10-22 (due 10-28, a document only). Numbers: the compile ledger
`ledger-c4fade734-O2.txt` (d_faces -O2, callgrind Ir, ours 1318 M against the pin's 509 M, 2.59x) and the
coordinator's MemorySSA findings (w-1's #3471 plan).

## What costs

- `Annotated` is eager: `manager::Annotated` (manager.rs:865) makes a `MemRef` (the decomposition of the address, its provenance, its
  roots) for every load and store of a function, from `PointerValues`, `Registers`, `Shape`, `AssumptionCache`, `ExposedFrames`,
  `Counted`, `DominatedEdges` and the loop bounds. d_faces -O2: 89 builds, 93.3 M Ir under `fold::Fold` (the llrm-only bucket, 225 M).
- `Accesses::managed` (memoryssa.rs) builds MemorySSA from `Annotated` and `CallEffects`: 33 builds in a d_faces -O2 compile (EarlyCSE 22,
  DSE 3, MemCpyOpt 11 asks), 51.5 M, of which 41.8 M in `alias::calls_annotated` (`points_to` 15.5 including the escape solve, 88 solves
  17.5; `_unknown_base` 10.3; `SliceSet::absorb` 5.5). The pin spends 12.7 M on the same three passes.
- Outside the scalar passes: `homes::Homes` 13.4 M (`Accesses::managed` 10.2 for 6 functions), `spares::Spares` 11.4 M (`managed` 11.0
  for 2 calls): 21.2 M of eager accesses for functions that may have no candidate.
- Callers of `Annotated`: memoryssa.rs:345, :392 (the accesses and their update); manager.rs:1350, :1364 (`PlainAccesses`), :1476, :1533
  (`ThroughMemory`, `FloatFacts`), :1814 (`Alias`), :1919 (the prefetch); licm/mod.rs:784 keeps it across a loop edit. Callers of
  `Accesses::managed`: earlycse.rs:1168, deadstore.rs:163, licm/mod.rs:367, promote.rs:488, homes.rs:116, spares.rs:123.

## What the pin does

MemorySSA is built once per function, eagerly, from instruction kinds alone
([`buildMemorySSA`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSA.cpp#L1522): a Def for each
instruction that may write, a Use for each that only reads); the alias queries come after, lazily, per load and bounded
([`OptimizeUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSA.cpp#L1307) and the
[`ClobberWalker`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/MemorySSA.cpp#L484)), each answer cached in the
`AAQueryInfo` ([`CacheEntry`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Analysis/AliasAnalysis.h#L242)). BasicAA
decomposes an address when a query names it
([`DecomposeGEPExpression`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L615), called from
[`aliasGEP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L1118)), not for every access.
EarlyCSE, DSE and MemCpyOpt keep MemorySSA up to date and hand it on instead of rebuilding it (w-1's lever B).

So the pin's MemorySSA needs no per-instruction provenance up front: structure from opcodes, provenance from the query.

## The target

1. The graph's structure from opcodes and call attributes alone: `Accesses` classifies each instruction (Def, Use, none) without a
   `MemRef`; a call's footprint is asked of `CallEffects` only for calls (it already is the per-call solve).
2. `Annotated` lazy: the `MemRef` of an instruction made when a walk or a pass asks, memoized per instruction under the function's
   mark, dropped by the same `depends` contract as today. `alias::annotated_with` gains a per-instruction entry point; the eager map is a
   `LazyMap` over it. A client that wants all of them (a few do: the table builders in `manager.rs`) asks for all.
3. One `calls_annotated` solve shared by the accesses and the call effects (w-1's lever 1); moot once #3436 deletes `CallEffects`: the
   order is #3436 first.
4. Homes and Spares ask for accesses only of a function with a candidate (a loop and a fixed-cell load for Spares: w-6 plans that part
   10-29: one of us, not both).
5. The escape and `_unknown_base` memo of the pin's capture cache (lever C, alias.rs, after 11-09).

## Order and the check on each

Each step is a PR with the same three lines: Ir before and after on d_faces -O2 (callgrind, `tools/pairs/board.py`'s arms line, binaries
built in my own worktree, HEAD == sha checked), objects identical (`tools/pairs/objsweep.py`, the 568-object sweep, plus the QB programs),
`LLRM_CHECK_CLOBBERS` and `LLRM_CHECK_REPLAY` clean.

| step | what | files | date |
|---|---|---|---|
| A | `Annotated` lazy per instruction (2), and carried across edits by the change set (A', below) | manager.rs, alias.rs | 11-04, after w-8's 11-02 `_numbers` deletion (Fold's callers go first) |
| B | `Accesses` without `MemRef`s (1): `PlainAccesses::run` 15.0 M | memoryssa.rs | first code 11-06, first Ir drop 11-06 |
| C | Homes/Spares fetch accesses per candidate function (4), with w-6 | homes.rs, spares.rs | 11-09 |
| D | #3436 (delete `CallEffects`) lands; `Writes` (17.9 M) then reads the one solve | alias.rs, manager.rs | first code 11-02 (rebase, Ir measured), first Ir drop with it |

A' (2026-10-22, from w-3's licm-compile-cost.md): `PlainAccesses::update` (manager.rs:1339) fetches all of `Annotated` and `Writes` to compare them with the held ones, 39 calls, 43.5 M of d_faces -O2's 1435 M, so a lazy map alone is defeated by the comparison. `Annotated` is therefore updated from the change set (only instructions whose address inputs changed are re-annotated; the keep-or-drop contract of #3518). Covers at most `Annotated`'s 24.7 M; `Writes` (17.9 M, a `CallEffects` solve) goes with D and `PlainAccesses::run` (15.0 M) with B. Measured in the PR, not estimated.

Not in this plan: w-1's lever B (EarlyCSE, DSE and MemCpyOpt preserve MemorySSA, 11-04, their files); `ThroughMemory`'s calls from
Fold, which w-8's `_numbers` deletion (11-02) removes first.

## Risks

- A lazy `MemRef` changes when it is made, not what it says: the objects must stay identical; a result that depended on the eager order
  (an `IndexMap` iteration over all of them) is found by the sweep and by `LLRM_CHECK_STALE`.
- `Annotated` is kept past edits by `update`/`remapped` (the arrprm lesson: #3518); the lazy table must carry the same contract, entry by
  entry: an entry made before an edit is kept or dropped as the whole map is today.
- The ledger's 93.3 M is inclusive of the analyses `Annotated` asks for (`PointerValues`, `Registers`...): the gain is the share that is
  not asked for by something else too; step A states it measured, not estimated.
