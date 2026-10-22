# SimplifyCFG + JumpThreading + CVP compile cost: ranked callees

Ledger e90b803c8a bucket: ours 90.9 M Ir against the pin's 33.6 M (2.70x) on d_faces -O2.

Method: callgrind, `llrm-c -m32 -mabi=sysv -march=i486 -O2` on `render/d_faces.c` (release, parity 1bd7d34afc + #3660; `/tmp/cg.sw.out`) against the pin's `opt -passes=default<O2>` callgrind on the same stage (`~/scratch/ledger2/cg-opt.out`), inclusive Ir, `callgrind_annotate --tree=calling`.

## Pass totals

| pass | ours | pin | ratio | excess |
|---|--:|--:|--:|--:|
| SimplifyCfg (all runs) | 37.06 M | 8.58 M | 4.3x | 28.5 M |
| CVP | 31.11 M | 9.51 M | 3.3x | 21.6 M |
| JumpThreading | 21.34 M | 7.21 M | 3.0x | 14.1 M |
| LazyValueInfo `solve`, under both CVP and JT | 29.72 M | 9.90 M | 3.0x | 19.8 M |

LVI is inside the CVP and JT totals, so the three passes' excess is 64 M and about 20 M of it is LVI.

## Ranked callees (ours, inclusive)

1. **LVI `solve`** 29.7 M: `get_edge_value` 19.1 M in 12279 calls (1560 Ir each; the pin's `getEdgeValue` is 6.9 M in total), `predecessors` 2.8 M (10783 calls: a vector built per call), `insert_result` 2.3 M. Cause to confirm: edge values recomputed per query where the pin's cache holds them.
2. **CVP's LVI queries** 21.6 M: `get_constant_on_edge` 9.3 M (593 calls), `get_constant_range_at_use` 6.4 M (574), `get_constant_at` 4.4 M (212 calls, 20.8 K each), `get_predicate_at(_values)` 5.8 M. 15.8 K Ir per `get_constant_on_edge` call against the pin's 5.5 M `getConstantOnEdge` in total.
3. **SimplifyCfg `loop_form_blocks`** 6.4 M in 34 calls (188 K each): rebuilds the loop analysis per call; lever 1 (analysis keyed on `function.epochs().cfg`). Upper bound measured earlier: skipping it entirely is -15.8 M (-1.26%).
4. **SimplifyCfg per-block visitors**, about 3844 blocks visited: `branchtocommon::folded` 2.9 M, `duplicate_phis_in` 2.2 M, `undefpred::removed` 2.0 M, `Function::predecessors` 1.8 M (10384 calls), `icmpchain::folded` 1.6 M, `merged_into_predecessor` 1.5 M, `condtocond::simplified` 1.4 M, `twoentryphi::folded` 1.4 M: 500 to 750 Ir per call on blocks they do not change. Cause to confirm: each visitor rebuilds its own view of the predecessors.
5. **JumpThreading `process_threadable_edges`** 12.3 M in 687 calls (17.9 K each): its LVI use is counted under 1 and 2; the rest is `compute_value_known_in_predecessors` 16 M inclusive.

## Plan

Each item is a code PR with Ir on both arms (callgrind d_faces -O2) and identical objects (`objsweep.py`, rows, QCport); owner w-4.

| item | change | target | date |
|---|---|---|---|
| 3 | `loop_form_blocks` analysis cached on the cfg epoch | -6 M of 34 M | 2026-10-30 |
| 1 | LVI: edge-value cache and `predecessors` without a per-call vector | -10 M of 29.7 M | 2026-11-06 |
| 2 | CVP: `get_constant_at` and range-at-use queries share LVI's cache | -8 M of 31 M | 2026-11-13 |
| 4 | SimplifyCfg: one predecessor view per iteration for the visitors | -8 M of 34 M | 2026-11-20 |
| 5 | JumpThreading after 1 and 2: re-profile | open | 2026-11-27 |

The targets are guesses from the callee costs until each is profiled in isolation; the PR states what it measured.
