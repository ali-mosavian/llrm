# SimplifyCFG + JumpThreading + CVP compile cost: ranked callees

Ledger e90b803c8a bucket: ours 90.9 M Ir against the pin's 33.6 M (2.70x) on d_faces -O2.

Method: callgrind, `llrm-c -m32 -mabi=sysv -march=i486 -O2` on `render/d_faces.c` (release, parity 1bd7d34afc + #3660; `cg.sw.out`) against the pin's `opt -passes=default<O2>` callgrind on the same stage (`cg-opt.out`), inclusive Ir, `callgrind_annotate --tree=calling`.

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

## Re-take after #3792 (callgrind d_faces -O2)

- **Lever 1 (#3792):** `loop_form_blocks` remembered per graph: -1.41 M Ir, objects identical. 24 of 34 asks still miss (the CFG changed since the last); what is left of it is 5.2 M, of which `loops_of` is 2.0 M (84 K per call: `DomTree::of` 0.86 M, `natural` 0.62 M, `LoopInfo::analyze` 0.53 M). A cheaper derivation is worth at most about 1 M: not the lever the plan guessed.
- **LVI `get_edge_value_local`:** ours 13,831 calls at 1,153 Ir each (15.9 M); the pin 8,593 calls at 498 Ir (4.28 M). So 1.6x the asks and 2.3x the cost of each. The cost is flat: `get_value_from_condition` 11,297 calls at 1,150 Ir (`get_value_from_icmp_condition` 0.8 K per call, `match_icmp_operand` 20,627 calls), no one function above 1.3 M self.
- **Allocation:** `__rust_alloc` 44 M, `memcpy` 41 M and `__rust_dealloc` 27 M are 7% of the whole run; the lattice and range values are cloned per query.
- **Next:** cut the asks first (the per-block visitors of SimplifyCfg ask LVI for edges `knownpred` and `jumpthreading` already asked), then the clones in `get_edge_value_local`; each as its own PR with Ir on both arms and identical objects.

## Allocation sites (callgrind d_faces -O2, parity 76db1d41dd)

`__rust_alloc` 44.1 M Ir, `memcpy` 41.1 M, `__rust_dealloc` 27.1 M: 7% of the run. By caller (Ir inside the allocator, calls):

| M Ir | calls | site |
|--:|--:|---|
| 8.12 | 308,659 | `RawVec::finish_grow` (a `Vec` grown one push at a time): 10.6 M of it under `do_reserve_and_handle`, 5.4 M `RawVec<u32>::grow_one` (72 K), 3.4 M `IndexMap<InstId, MemRef>::insert_full` (455 inserts: one map regrown) |
| 3.05 | 116,854 | `String::clone`: 2.9 M from `Semantics::clone` (39,542), 1.2 M from `Vec<String>::clone` |
| 1.55 | 64,200 | `Vec<Loc>::clone` (`Semantics` operands) |
| 1.12 | 27,592 | `HashMap::with_capacity` |
| 0.98 | 40,852 | `peep::walk::windows` |

Two sites that name their cost: `masm::frame_omitted` clones the whole item list 11 times (5.65 M inclusive: `stack_addressed` does `items.clone()` for each pass of its retry loop), and `peephole::_register_effects_worked` clones a `Semantics` as the cache key 5,559 times (5.8 M inclusive, `EffectsHeld::remember`). `Semantics` carries a `String` name and two `Vec<Loc>`: every clone of an `Insn` is three allocations.

Next, each its own PR with Ir on both arms and objects identical: `Semantics` names as `&'static str`/`Arc<str>` (removes the String clones), the key of `EffectsHeld` by reference, `stack_addressed` without the per-pass clone, and `with_capacity` at the `RawVec<u32>` pushes.

## Closed: the flat tail (2026-10-24, parity 4b9b80871f)

- `RawVec<u32>::grow_one` is 1.69 M Ir inclusive (the 5.4 M above was `finish_grow`'s share), in a dozen callers of at most 0.57 M each (`exposed_allocas`, `LiveIntervals::new`, `push_operand`, `references::of`): a `with_capacity` at each is about 1 M.
- The SimplifyCfg visitors are 0.4 to 1.4 M each after #3834; `occurrence::arms` allocates 2.6 M in 14 callers, and `undefpred::removed` mutates while it walks them, so no iterator replaces the `Vec`.
- LVI: the asks equal the pin's (593 / 574 against 516 / 475), the edge evaluations are 1.85x (12.3 K against 6.6 K), `get_edge_value_local` is a faithful port (1,153 against 498 Ir a call, flat); sharing CVP's `LviCache` with JumpThreading moved nothing (+37 K Ir, objects identical) and was not landed.
- Landed: #3792 (-1.41 M), #3834 (-3.62 M), #3871 (-1.86 M), #3872 (-3.43 M). Open: `Vec<Loc>` clones in `Semantics` (11 M inclusive), 2026-11-06.

## Dated plan at parity 00a8ea652 (2026-10-25)

callgrind d_faces -O2 on the ledger command (release), ours against the pin's `opt` (cg-opt.out):

| pass | ours | pin | excess | ratio |
|---|--:|--:|--:|--:|
| SimplifyCfg, 99 `iterativelySimplified` (the pin: 80) | 33.5 M | 8.58 M | 24.9 M | 3.9x |
| CorrelatedValuePropagation, 22 runs | 29.0 M | 9.51 M | 19.5 M | 3.0x |
| JumpThreading, 22 runs | 19.6 M | 7.21 M | 12.4 M | 2.7x |
| of which LVI `solve` (under CVP and JT) | 27.3 M | 9.90 M | 17.4 M | 2.8x |

Steps, one PR each, Ir on both arms and identical objects, the pin pass each mirrors named:

| # | step | mirrors | target | date |
|--:|---|---|--:|---|
| 1 | `loop_form_blocks` from the manager's cached loop analysis (4.9 M in 36 asks, 137 K each) | `NeedCanonicalLoop` via `LoopInfo` | -3 M | 2026-11-12 |
| 2 | the per-block visitors take `sole_predecessor`/`has_n_predecessors` in place of `predecessors()` (9,955 calls, 1.7 M) and `branchtocommon::folded` asks the cheap shape first (2.7 M, 3,661 calls) | `simplifyCFG` / `foldBranchToCommonDest`'s early exits | -2 M | 2026-11-17 |
| 3 | LVI: one evaluation of a branch condition for all the values asked of it in a solve (`get_value_from_condition` 11 K calls at 1.15 K Ir against the pin's 498) | `getValueFromCondition` with its `ValueLatticeElement` cache | -6 M | 2026-11-24 |
| 4 | CVP: `get_constant_on_edge` (528 calls, 14.8 K Ir each) and `get_constant_range_at_use` (554) reuse the block-value cache across the processed blocks | `CorrelatedValuePropagation::runImpl` order | -4 M | 2026-12-01 |
| 5 | JumpThreading `process_threadable_edges` (619 calls, 17 K each): `computeValueKnownInPredecessors` memo per (value, block) | `computeValueKnownInPredecessorsImpl` | -3 M | 2026-12-08 |
| 6 | re-take on the ledger (dist), the remaining top-5 callees by Ir | - | - | 2026-12-10 |

Each target is a guess from the callee cost until the step is profiled; the PR states the measured number.
