---
architecture-index: ConstraintElimination as a translation of LLVM's
architecture-order: 275
---
# ConstraintElimination as a translation of LLVM's: the plan

TL;DR: `ConstraintSystem.h/.cpp` (Fourier-Motzkin on sparse rows) is translated into `crates/ir/llrm-mir/src/constraintsystem.rs`; the pass `ConstraintElimination.cpp` follows and replaces `difference` (a Floyd-Warshall closure, O(n^3), 24 M on d_faces) and the `guards::Given::holds` path (12 M).

Rule for the code: each Rust function is a translation of the C++ function named in its comment (permalink at `d1106deb71cc`); anything not translated is a numbered DEVIATE in the module header.

## 1. Stages

| stage | what | oracle |
|---|---|---|
| C1 | `ConstraintSystem`: rows, `addRow`, `mayHaveSolution` (FM with the 500-row give-up), `getSubSystem`, `isConditionImplied`, `isConditionImpliedInSubSystem` | `tools/llvmport/constraintsystem_dump.sh`: 17,184 hashes of systems and queries over the pinned LLVM, including i64-overflow rows and the give-up |
| C2 | the pass: `decompose` over MIR values, `ConstraintInfo`, `State`, the dominator-tree DFS (`eliminateConstraints`), `checkAndReplaceCondition` | difftest against `opt -passes=constraint-elimination` on the corpus |
| C3 | migrate the consumers of `difference::proves` and `guards::Given::holds` (`decide`: jumpthread, loopfuse, decide) | identical.py, then `difference` has no caller and is deleted |

`guards::holds` for trip-count guards (induction, exitfold) is SCEV's (`isLoopEntryGuardedByCond`, scev-port), not this row's.
