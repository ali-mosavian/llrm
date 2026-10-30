# Early Tail Duplication: the translation

Pin d1106deb71cc, [TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp) and TailDuplication.cpp (`EarlyTailDuplicate`, `PreRegAlloc = true`, `LayoutMode = false`). Slot-table row 34, registry `early-tailduplication` (Missing). Owner w-1.

## Where it runs
Directly after instruction selection, before OptimizePHIs (`llc -O2 -debug-pass=Structure`: Finalize ISel, X86 Domain Reassignment, Early Tail Duplication, Optimize machine instruction PHIs). On SSA, phis in blocks. Ours: the first LIR phase of `flow.rs`, before `OptimizePhis`.

## What it does
`tailDuplicateBlocks` walks the blocks; `tailDuplicateAndUpdate` copies a small block into each predecessor that ends in an unconditional jump to it (`tailDuplicate`: `duplicateInstruction`, `processPHI`, `updateSuccessorsPHIs`), then repairs SSA for the values the copied block defined and later blocks read (`addSSAUpdateEntry`, `MachineSSAUpdater`), and deletes the block when no predecessor is left (`removeDeadBlock`).

| Pin function | Lines | Ours |
|---|---|---|
| `shouldTailDuplicate` | 563-719 | the layout-mode half exists (`jumps/placement/taildup.rs`); the pre-RA rules are new: `TailDuplicateSize` 2 (1 at -Os), `TailDupIndirectBranchSize` 20, no call or return before RA, pred/succ limits 16, `canCompletelyDuplicateBB` |
| `isSimpleBB`, `canCompletelyDuplicateBB`, `bothUsedInPHI` | 721-755 | new |
| `canTailDuplicate` | 829-861 | the placement version is post-RA; new |
| `tailDuplicate` and `duplicateSimpleBB` | 757-1069 | new (copies carry fresh value numbers; phis of the tail become copies or are renamed per predecessor) |
| `processPHI`, `duplicateInstruction`, `updateSuccessorsPHIs`, `appendCopies` | 351-561, 1071 | new |
| `addSSAUpdateEntry`, the SSA repair | 336-349, 166-273 | new: LIR has no SSA updater (MIR has `SSAUpdater`); one small one over `Phi`, block successors and the dominator tree |

## What LIR gives it
`LirBlock` has `phis` (`Phi { result, incoming: Vec<(pred, value)> }`), `succ`, `odds` per edge, and unique values numbered by `next_value`. A copy needs new block-local values and, for a duplicated instruction, the new defs; a new edge pred -> succ needs its odds.

## Steps (one PR each, fail-first from a hand-built LIR body, pinned against `llc -stop-after`)
1. 2026-10-27 (#4561): `shouldTailDuplicate` pre-RA + `canCompletelyDuplicateBB` as a function over a body; unit tests on the pin's cases (a diamond's join, an indirect branch, a call, -Os).
2. 2026-10-29 (this step, done 10-25): `tailDuplicate` of a tail with no phis and no value read outside it (the pin's simplest case: copy the instructions into the predecessor in place of its jump), odds updated.
3. 2026-11-01: tails with phis and successors with phis (`processPHI`, `updateSuccessorsPHIs`) and the SSA update; the registry slot (parity row `early-tailduplication`) and slot-table row 34 become MATCH or a numbered DEVIATE.
4. Ledger after each: objsweep, QCport, the 71 bench programs under both selectors (the pin's effect on its own bench is 0.008% of clocks: row 34), and what it does for `ret` duplication (STRISTMP-1).
