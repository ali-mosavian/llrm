# Compile time: what was measured and what changed

TL;DR: instructions executed by the compiler, QCport at -O1/-O2/-Os, are 61-63% of what they were before this series (geomean 0.68), the bench programs 69-71%. The table is the whole of main, not only these PRs: mimalloc (-7 to -11%) and the other sessions' work are in it.

## Before and after

Compiler instructions (`LLRM_DEBUG=time`, `[instr] total`), the parent of #1041 against main. Total is the ratio of the sums; geomean is over the files; worst is the file that gained least.

| group | level | before (G) | after (G) | total | geomean | worst file |
|---|---|---:|---:|---:|---:|---|
| bench (32) | -O1 | 7.3 | 5.0 | 0.692 | 0.690 | lru.c 0.756 |
| bench (32) | -O2 | 8.6 | 5.9 | 0.688 | 0.694 | lru.c 0.758 |
| bench (32) | -Os | 7.6 | 5.4 | 0.707 | 0.709 | lru.c 0.758 |
| qcport (65) | -O1 | 375.0 | 230.1 | 0.613 | 0.679 | client.c 0.769 |
| qcport (65) | -O2 | 453.2 | 283.3 | 0.625 | 0.683 | client.c 0.765 |
| qcport (65) | -Os | 454.7 | 284.5 | 0.626 | 0.683 | client.c 0.763 |

Generated programs (the scaling axes of the gate), -O2: a loop nest 16 deep 7.35 G to 2.75 G, 32 deep 70.1 G to 23.2 G, a chain of 32 callers 1.30 G to 0.94 G.

## Landed

Each is a pull request of this series; every one is byte-identical on the 272 programs checked (bench, kernels, QCport) with its `LLRM_CHECK_*` switch on, unless it says otherwise.

- [#1041](https://github.com/ali-mosavian/llrm/pull/1041) the runtime table is Rust data from the build, parsed by no process.
- [#1043](https://github.com/ali-mosavian/llrm/pull/1043) `IdMap`/`IdSet`: the spill forecast's spilled set is a bit set.
- [#1046](https://github.com/ali-mosavian/llrm/pull/1046), [#1047](https://github.com/ali-mosavian/llrm/pull/1047) calls reaching the same bytes share one list of references, resolved once.
- [#1057](https://github.com/ali-mosavian/llrm/pull/1057), [#1058](https://github.com/ali-mosavian/llrm/pull/1058), [#1064](https://github.com/ali-mosavian/llrm/pull/1064), [#1069](https://github.com/ali-mosavian/llrm/pull/1069), [#1071](https://github.com/ali-mosavian/llrm/pull/1071) ranges: a loop block's facts come from the loop's swept ones and the prefix of edges it shares, not from sweeping every operation; every pass asks the manager's `Bounded`. Took the cost of a nest from cubic in its depth.
- [#1060](https://github.com/ali-mosavian/llrm/pull/1060) the guards on entry to a block are found once for the questions asked of it.
- [#1067](https://github.com/ali-mosavian/llrm/pull/1067) hoist: a motion's price and its spill set come from one forecast.
- [#1072](https://github.com/ali-mosavian/llrm/pull/1072), [#1078](https://github.com/ali-mosavian/llrm/pull/1078), [#1119](https://github.com/ali-mosavian/llrm/pull/1119) lsr: loops that change nothing share what is found of the function; frequencies read the manager's counted proofs; a block's liveness is walked once, not once a use (lsr -37% on `d_faces` at -O1).
- [#1074](https://github.com/ali-mosavian/llrm/pull/1074) a `LLRM_CHECK_*` switch is a lookup in a snapshot of the environment, not a `getenv`.
- [#1084](https://github.com/ali-mosavian/llrm/pull/1084) `SparseIdMap` names the id-keyed maps that hold few of many.
- [#1091](https://github.com/ali-mosavian/llrm/pull/1091), [#1095](https://github.com/ali-mosavian/llrm/pull/1095) one dataflow solver for the block-level fixed points: a block is worked again only when an input changed (lir peephole on an 8-deep nest -40%).
- [#1101](https://github.com/ali-mosavian/llrm/pull/1101) branch probability: an edge's trips are worked out once (lir jumps on a 16-deep nest -27%).
- [#1104](https://github.com/ali-mosavian/llrm/pull/1104) a store moved to a loop's exit keeps the ranges and facts held (a 32-deep nest -23%).
- [#1115](https://github.com/ali-mosavian/llrm/pull/1115) -O1 states no parameter ranges from callers' arguments (gcc's `-fipa-vrp` is -O2; -1.0%).
- [#1122](https://github.com/ali-mosavian/llrm/pull/1122) docs/levels.md: the passes gcc leaves to -O2 that ours runs at -O1, each with its `opts.cc` line.
- [#1128](https://github.com/ali-mosavian/llrm/pull/1128) the change log is filtered against what each analysis declares it reads (`Depends`); `LLRM_WHY` counts recomputations (-0.6%).
- [#1134](https://github.com/ali-mosavian/llrm/pull/1134) `LLRM_DEBUG=runs` reports a pass's own work apart from the analyses it computed first (the earlier reading of 63% idle was 6.1% of the compile).
- [#1142](https://github.com/ali-mosavian/llrm/pull/1142) the constant-cycle propagation is set up only where a phi has no fact (-0.9%).
- [#1153](https://github.com/ali-mosavian/llrm/pull/1153) the module summaries are frozen after the interprocedural step; passes that add no memory operation read them (-1.7%); `LLRM_CHECK_STALE` runs in the gate.
- [#1162](https://github.com/ali-mosavian/llrm/pull/1162) `initialized` does not solve a body that writes no pointer parameter (geomean -1.9% at -O1, total -0.3%).

## Parked

- [#1166](https://github.com/ali-mosavian/llrm/issues/1166) per-call incremental call-effects and points-to: the family is 23% of the -O1 compile and its bodies really change between passes (4 of 90 reuse checks found a body untouched); a redesign for 1-2%.
- [#1167](https://github.com/ali-mosavian/llrm/issues/1167) alias pair memo across states: a memo keyed by content is +10%, one per `Accesses` is +1.0%; needs ids stable across states.
- [#1168](https://github.com/ali-mosavian/llrm/issues/1168) `consts::cells` clones its maps in the fixed point: holding them only at loads gave -0.1%; needs a persistent map and a changed/cloned count first.
- [#1169](https://github.com/ali-mosavian/llrm/issues/1169) `_pointer_stores` builds a MemorySSA per memory solve: the graph must stop borrowing its `Unit` to be shared.
- [#1170](https://github.com/ali-mosavian/llrm/issues/1170) the interprocedural rerun loop (gcc's early inliner iterates once): a quality question, not a compile-time one.
- [#1106](https://github.com/ali-mosavian/llrm/issues/1106) a value-numbering-only mode at -O1 (gcc runs FRE at -O1): quality gap, not started.
- [#1105](https://github.com/ali-mosavian/llrm/issues/1105) the fixed-point loops not yet on the dataflow solver, and why the alias/summary thread is closed.
- [#1088](https://github.com/ali-mosavian/llrm/issues/1088) the spiller route runs the machine phases twice (regparm16's allocator).
- [#1110](https://github.com/ali-mosavian/llrm/issues/1110) loop passes cost O(depth^2): under 0.1% on real code, found on the synthetic axis.
- Tried and dropped, no issue: decide on the manager's `ThroughMemory` (-0.4%, and the gate refused it: through-memory grows superlinearly on the callers axis); a shared `points_to` for GlobalsAA, Summaries and stamp (the three read different Units; the identical-input repeats followed real edits; -0.23%).
