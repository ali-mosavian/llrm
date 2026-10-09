# Module analyses after an edit: recompute what depends on it

TL;DR: every inline, specialisation or revert in `interprocedural.rs` drops all module analyses and the next consumer recomputes GlobalsAA and Summaries over every body. On QCport's `host.c` 72 summaries runs are 20.8 s of the 35.8 s MIR pipeline; call-effects another 2.4 s. After editing body `f` only `f` and what reads it can have changed. Proposal: keep the previous results and a dirty set, recompute the dependent part of the call graph as LLVM's CGSCC manager does, and run the old whole-module recompute beside it under a check switch.

## Measured (-O2, instructions or own ms; the profile is of `host.c`)

| module | pipeline | summaries (runs) | call-effects (runs) | globals-aa | summaries direct |
|---|---|---|---|---|---|
| host.c | 35.8 s | 20.8 s own + children (72) | 2.4 s (582) | 0.75 s (72) | 0.72 s |
| d_alias.c | 4.6 s | 0.72 s (19) | 0.29 s (267) | 0.23 s | 0.22 s |
| mdl.c | 1.0 s | 0.32 s (16) | 0.10 s (279) | 0.07 s | 0.04 s |

`host.c` profile: 30% in `MemoryObject::cmp` (the `Slice` sets in the summaries worklist), 14,629 worklist visits in 72 runs. The kernels (66 programs) spend 0.8% in summaries; this is a real-program cost, QCport is ~10x gcc at -O2.

## What a splice drops today

All callers pass `PreservedAnalyses::none()` (`interprocedural.rs` 228-234 `tried_sites`, 330-360 `trial`, 409-414 `edited`, 633-660 `stamped_all`; `pipeline.rs` `rerun`). `ModuleAnalyses::changed(id)` only drops body `id`'s function analyses. The next `rerun` calls `analyses.outer(module)`, which recomputes CalleeEffects, CallRegisters, GlobalSizes, Declarations, TypeAncestry, GlobalsAA and Summaries, and keeps the old `Rc` only when the new result compares equal (`computed`, `Outer::same`). So the result is reused when nothing changed, but its cost is paid every time.

## LLVM's answer, and the part we copy

LLVM does not recompute a module-wide summary per edit. The CGSCC pass manager updates `LazyCallGraph` in place after each edit and invalidates the analyses of the functions it changed (`FunctionAnalysisManagerCGSCCProxy`), and function-attribute inference runs bottom-up over the SCCs that the edit touched. A module analysis that a pass does not preserve is recomputed, as ours; the structure that keeps it cheap is that the expensive per-function work is keyed by function and by SCC. (From memory of LLVM's design, not read in source for this note.)

## Design

`ModuleAnalyses` gets `dirty: BTreeSet<GlobalId>`, filled by `changed(id)`, which every call site already calls, and cleared when the analysis that consumes it has been brought up to date. A module analysis may implement `update(previous, dirty, ...) -> Option<Result>`, defaulting to `None` (recompute), as `Analysis::update` does for functions; a result it makes must equal what `run` makes. `LLRM_CHECK_MODULES=1` recomputes in full and asserts equality, as `LLRM_CHECK_REPLAY` does.

1. Summaries (the cost). Entry `f` depends on `f`'s body, `f`'s callees' entries, GlobalsAA, and `callbacks` (the merged summary of the module's entries, read by every body that calls something unknown). So after editing a set D of bodies: the dirty closure is D, every transitive caller of D (the existing `readers` graph), and, if any entry's body is in the closure, every caller of something unknown. Everything outside the closure keeps its previous summary, its `direct` summary and its `Visit` memo; the closure is reset to its `direct` start (captures empty, the least fixed point) and the existing worklist runs over it alone. This is exact: a body outside the closure reads nothing inside it. It falls back to the full run when GlobalsAA changed by value (every `direct` summary reads it), when a body was added or removed, or when the previous result is missing.
2. GlobalsAA. Its result folds per-body contributions (escaped globals from `points_to`, names held in constants, references for `entries`); `held` is a reachability fold over initialisers. Cache the three per-body sets keyed by `GlobalId` and body version, recompute the dirty bodies' and refold. The result changes by value rarely, and when it does Summaries takes its full path.
3. Declarations, `Outer::same`, CalleeEffects: update entry `f` only, and compare entry `f`, instead of cloning and comparing the metadata and every declaration per `outer()` call.
4. TypeAncestry, GlobalSizes, CallRegisters read no function body: preserved by `changed(id)` (`PreservedAnalyses::preserve_module`).

ProgramSummaries is left alone: it is read stale inside the interprocedural loop by design.

## Order and measurement

1 first (host.c: summaries 64% of the pipeline), then 2, then 3-4. Each its own PR, objects identical, with the 97-program table, host.c beside it, and the check switch run over QCport, bench and tests/run. Fail-first test per PR: the number of body visits and `direct` summaries after one edit (counters `summaries rounds`, `DIRECT_RUNS`) is proportional to the closure, not to the module.

Also open and independent: the `MemoryObject` ordering (30% of host.c's profile) — an interned id would make the sets cheap; not part of this change.
