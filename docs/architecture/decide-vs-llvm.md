---
architecture-index: decide against SimplifyCFG and JumpThreading
architecture-order: 140
---
# decide against SimplifyCFG, JumpThreading, CorrelatedValuePropagation and ConstraintElimination

TL;DR: `decide` is 124.7 M of `d_faces` -O2 (36 runs, 3.5 M a run) and decides nothing: all 36 runs find no branch to decide, 4 of them thread a block. Across the 38 bench programs it decides 15 branches in all (guards 10, constants 4, ranges 1) in 5 runs of about 520. LLVM's four counterparts together are 31.9 M. The cost is per run and per branch, spent on answers that are all "no": 1,021 range proofs that fail, 486 guard collections, a constant propagation over the whole body, and the facts asked first (58%).

Sources: llrm CI-mode build of main 7ac662405, `llrm-cc -m32 -O2` of `d_faces.c`, callgrind inclusive of `decide::_decided` and its callees (children overlap where a callee is asked first by `decide`); decisions counted by a probe in `_decided` and `_decisions` (throwaway); LLVM 24 `opt -passes='default<O2>'` on the same file, inclusive of `<Pass>::run`.

## 1. LLVM, the four parts

| LLVM | M | runs | what |
|---|--:|--:|---|
| SimplifyCFG | 10.7 | 78 | folds a branch on a constant, merges blocks, removes empty ones, threads a block that only branches |
| JumpThreading | 7.4 | 22 | threads a block over a branch whose condition a predecessor decides (`ProcessBranchOnPHI` too) |
| CorrelatedValuePropagation | 9.5 | 22 | decides a compare from `LazyValueInfo` ranges on the edge |
| ConstraintElimination | 4.3 | 11 | decides a compare from the dominating conditions, in one scoped walk of the dominator tree |
| SCCP (constants) | 4.4 | 11 | |

31.9 M for the five, 144 runs. `decide` is the four in one pass with a fixed order and the facts of the whole body asked first.

## 2. What `decide` spends, per part

`_decided` 124.7 M (inclusive, `d_faces` -O2, 36 runs):

| part | M | calls | per call | LLVM counterpart |
|---|--:|--:|--:|---|
| facts asked first: `ThroughMemory` 46.8, `Held::of` (`Annotated`, `PointerValues`, `Registers`, `Bounded`) 26.0 | 72.8 | 36 + 36 | 2.0 M, 0.7 M | none: the facts other passes read (first asker pays) |
| guards: `Given::holds` 32.3 (`difference::proves` 19.3) + `Given::at` 11.0 | 43.3 | 972 + 486 | 33 K, 22.6 K | ConstraintElimination 4.3 |
| constants: `constant_cycles::propagated` over the body, executable edges only | 16.0 | 36 | 444 K | SCCP 4.4 |
| threading (SimplifyCFG part): `_threaded` 11.1 + `_phi_threaded` 4.6 (each builds the body's `Shape`) | 15.7 | 36 + 36 | 308 K, 128 K | SimplifyCFG + JumpThreading 18.1 |
| ranges: `on_edge_over` | 3.3 | 1,148 | 2.9 K | CVP 9.5 |

The per-part figures overlap by what `decide` asks first (the facts) and add to more than 124.7. The 1.5 M a run the LLVM-order experiment measures (own instructions, 82 runs) is this table without the facts.

## 3. What is named

- Every undecided conditional branch asks twice (a predicate and its inverse) of a `Given` built for the block: 1,021 queries reached `difference::proves` on `d_faces`, each building a difference system of at most 24 nodes with a cubic closure over `BigInt`s (33 K a query). 931 had guards, 90 none, 4 a declared range; none proved anything. ConstraintElimination keeps one constraint system and pushes and pops the conditions on the way down the dominator tree.
- `Given::at` climbs up to 64 dominators for every block and tests `alone` on each edge (22.6 K a block); the contribution of an edge to a block below it is the same for every block below.
- `constant_cycles::propagated` and the facts are whole-body work on every run, whether the body changed since the last one or not (the same body, 4 of the 36 runs changed it).
- The decision sources found: constants 4, guards 10, ranges 1 over the bench; guards are the one that finds most and, here, the one that costs most.

## 4. Rows, probes and kills

| row | M | change | probe | kill |
|---|--:|---|---|---|
| closure per query | 19.3 | machine integers for widths up to 64 in `proves`; one closure per `Given` where the system is the same for the second (inverse) question | share of `proves` time in `BigInt` operations; queries whose second question has the same system | under 6 M saved |
| guards per block | 11.0 | the edge contributions kept per (dominator, child) and the chain read, not rebuilt; same order | distinct (above, reached) pairs against steps climbed | repeat ratio under 3 |
| constants per run | 16.0 | none yet: the answer is unchanged when the body is | runs whose `propagated` result equals the previous run's on the same body version | under 50% repeat |
| threading | 15.7 | one `Shape` for both steps and for the run; a structural precheck before either (a block holding only a `br`; a phi-and-branch block) | runs where the precheck fails | under 8 M saved |
| the facts | 72.8 | not this pass: facts-per-query | | |

Order: closure (largest, exact), then guards, then threading. Each is exact (objects identical; the oracle for guards is the old `Given::at` under `LLRM_CHECK_`). Together about 25 M (1%) if the probes hold; the 58% of facts and the 82 runs of the LLVM-order experiment are the pass list's and the manager's.
