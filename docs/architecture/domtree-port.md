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

## 2. The oracle

`tools/llvmport/domtree_dump.sh` builds an 8-block graph (a loop, a diamond, a fan, an unreachable block, an infinite loop) in the pinned LLVM, applies every single edge flip, every pair and some triples as one `applyUpdates` batch, and prints a hash of each resulting `DominatorTree` and `PostDominatorTree` (immediate dominator, level and children in order, roots). `domtree_tests.rs` recomputes the 4,720 hashes from the port; all match. Premise assertions: the golden file names `LLVM 24.0.0git` and the commit `d1106deb71cc` (the test fails on any other binary), has one line per case and tree, and holds the cases where LLVM's updated post-dominator tree differs from a fresh one (267; none for dominators), so the port is checked against what LLVM does, not against a fresh build.

## 3. Readers and stages

One analysis owns dominance: the `Dominators` analysis in `llrm-mir`. Every reader asks it.

| stage | what moves | deletes |
|---|---|---|
| D1 (landed) | `DomTreeBase`, oracle | |
| D2 (with D1, one step) | `Dominators` result is `DomTree`; `Analysis::update` turns the change log into `Update`s for `applyUpdates`; readers in `llrm-mir` (`loopinfo`, `verify`, `valuetracking/*`, `scalarevolution`, `simplify`) and `cfg::Dominance` | `llrm-mir/src/dominators.rs`. Acceptance: `branches` and `unrolled` `analysis dominators` own slope within the limit at 2N |
| D3 | `graph::loops::{dominators, immediate_dominators, frontiers}` readers in `llrm-analysis` and `llrm-transforms` (`promote`, `mem2reg`, `gvn`, `sink`, `hoist`, ...) | the CFG-generic walks |
| D4 | LIR back end: `llrm-core` `analysis::loops`, `shrinkwrap`, `ssarepair`, `phiconst` | `llrm_core::analysis::loops::{dominators, immediate_dominators}` |
| D5 | `llrm-hir` `dominance.rs`, `llrm-support` `graph::dominance` | |
| D6 | `analysis dominated-edges` verdict | |
