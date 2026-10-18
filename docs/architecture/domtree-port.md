---
architecture-index: DominatorTree as a translation of LLVM's
architecture-order: 270
---
# DominatorTree as a translation of LLVM's: the plan

TL;DR: `GenericDomTree.h`, `GenericDomTreeConstruction.h` (Semi-NCA, `insertEdge`/`deleteEdge`/`applyUpdates`, `PostDominatorTree`), `CFGDiff.h` and `CFGUpdate.h` at `d1106deb71cc` are translated in `crates/ir/llrm-mir/src/domtree.rs` (D1, landed). The old dominator code is deleted as its readers move (D2 to D5). Issue [#2260](https://github.com/ali-mosavian/llrm/issues/2260) rows: `branches analysis dominators` x3.27, `unrolled analysis dominators` x2.48 and `cases dominated-edges` x2.42.

Rule for the code: each Rust function is a translation of the C++ function named in its comment (permalink at the pin); anything not translated is a numbered DEVIATE in the module header, resting on a stated fact about MIR. Deleting the code a translation replaces is the evidence it landed.

## 1. The translation

| LLVM | Rust |
|---|---|
| `DominatorTreeBase<NodeT, IsPostDom>` | `DomTreeBase<const POST: bool>`; `DomTree`, `PostDomTree` |
| `DomTreeNodeBase` | `TreeNode` (index addressed) |
| `GraphTraits` | `DomGraph` (`Function` implements it) |
| `SemiNCAInfo` | `SemiNca` |
| `GraphDiff`, `cfg::Update`, `LegalizeUpdates` | `GraphDiff`, `Update`, `legalize_updates` |
| `std::priority_queue` of `InsertionInfo` | `LevelQueue` (libstdc++'s heap) |
| `GenericDomTreeUpdater`, `DomTreeUpdater` (Eager, Lazy) | `domtree_updater::DomTreeUpdater` (D2b) |

## 2. The oracle

`tools/llvmport/domtree_dump.sh` builds an 8-block graph (a loop, a diamond, a fan, an unreachable block, an infinite loop) in the pinned LLVM, applies every single edge flip, every pair and some triples as one `applyUpdates` batch, as a Lazy `DomTreeUpdater` queue (one update at a time, and `applyUpdatesPermissive` with a repeat), and 46 critical-edge splits (Eager, Lazy), and prints a hash of each resulting `DominatorTree` and `PostDominatorTree` (immediate dominator, level and children in order, roots). `domtree_tests.rs` recomputes the 4,720 hashes from the port; all match. Premise assertions: the golden file names `LLVM 24.0.0git` and the commit `d1106deb71cc` (the test fails on any other binary), has one line per case and tree, and holds the cases where LLVM's updated post-dominator tree differs from a fresh one (267; none for dominators), so the port is checked against what LLVM does, not against a fresh build.

## 3. D2 measured

Own `analysis dominators` Minstr at N, 2N (and 4N for `branches`), release, `perf` counts, `tools/budgets.toml` sizes (`branches` 256, `unrolled` 8; limit x2.3):

| axis | main | D2 (#2271) | D2b (lazy, #2282) |
|---|---|---|---|
| branches | 25.5 -> 83.4, x3.27 | 25.6 -> 89.6 -> 332.4, x3.49, x3.71 | 6.6 -> 13.0 -> 26.0, x1.99, x2.00 |
| unrolled | 2.3 -> 5.7, x2.47 | 2.3 -> 5.2, x2.27 | 1.3 -> 2.6, x1.99 |
| cases | 9.2 -> 18.1, x1.98 | 7.7 -> 15.4, x2.01 | 7.7 -> 15.4, x2.01 |

D2 alone did not move `branches`: `rotate` asked for the tree after each of its N/8 loops and each ask applied three edge updates, and in a function whose loops follow one another every block after the edit is in the subtree LLVM's `DeleteReachable` and `InsertReachable` walk. Callgrind on `branches` (N=256, 512): `update_in_place` 22.0 M -> 83.5 M inclusive over 38 -> 70 calls, 578 k -> 1.19 M per call, so O(N) per update by LLVM's algorithm and the count of asks grows with N. The tree build is 1% of it.

D2b is LLVM's counterpart: a pass that edits the CFG in a loop holds a `DomTreeUpdater` with the Lazy strategy and the updates are applied when a reader asks. `cfg::Dominance` is that handle: `rotate` seeds `Shape.dominance` with the manager's tree and its mark (`Analyses::held`), the edits queue in the change log, and the first reader (`Dominance::tree(function)`) applies them as one batch through `DomTreeUpdater`. No reader of the seeded shape ran in the 1,021 transform tests or the 42 compiles; the output is byte-identical.

The readers that ask `Shape.dominance` now pass the function: `ranges`, `ranger`, `guards`, `induction`, `consts`, `avail`, `alias`, `slotstores`, `assumptions`, `manager` (`dominated_by`), `mssa` (resolved at build), `lsr`, `loadjoins`, `transform`.

## 4. Readers and stages

One analysis owns dominance: the `Dominators` analysis in `llrm-mir`. Every reader asks it.

| stage | what moves | deletes |
|---|---|---|
| D1 (#2268) | `DomTreeBase`, oracle | |
| D2 (with D1, one step), D2b | `Dominators` result is `DomTree`; `Analysis::update` turns the change log into `Update`s for `applyUpdates`; readers in `llrm-mir` (`loopinfo`, `verify`, `valuetracking/*`, `scalarevolution`, `simplify`) and `cfg::Dominance` | `llrm-mir/src/dominators.rs`. Acceptance: `branches` and `unrolled` `analysis dominators` own slope within the limit at 2N |
| D3-D5 (#2285) | the walks over blocks named by address: `llrm-analysis` `graph::loops`, the LIR back end (`analysis::loops`, `shrinkwrap`, `ssarepair`, `phiconst`, `copysink`), `llrm-hir` `dominance::check`: one `AddressDominance` over `DomTree` | `llrm-support` `graph::{Dominance, dominance, dominators, dominance_work, Dominates}` (the second copy of Cooper-Harvey-Kennedy) |
| D6 | `analysis dominated-edges` verdict below | |

## 5. `analysis dominated-edges`: KEPT

It is the edge-fact solver of `ranger.rs` (GCC's `ranger_cache` form: a value's interval at a block from the dominating edges, worked out when asked). LLVM's counterpart in role is `LazyValueInfo::getValueInBlock`/`getValueOnEdge` (6.2 M of clang's compile in the plan's analysis row), which no port owns, so there is no translation to replace it with and the verdict is KEPT, not REPLACED.

Its slope (`cases`, own Minstr N -> 2N: 2.9 -> 7.1, x2.42, limit x2.3) is not the solver's. Callgrind of `cases` 128 and 256: `update_owned` is 7.1 M, of which `Function::reach_of` is 6.1 M (85%; `keeping` 0.96 M); inside it `touched_with` goes 6.35 M -> 16.78 M (x2.64) as `IdSet<u32>::insert` goes 98.9 k -> 294.7 k calls (x2.98). The cost is the change-log digest every incremental analysis shares, so the fix belongs there (`touched.rs`), not in `ranger.rs`; it is its own stage.
