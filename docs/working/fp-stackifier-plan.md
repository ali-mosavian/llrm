# X86FPStackifier: compile-cost plan (row 9 of the machine clean-up plan)

Owner: w-2. Dated 2026-10-24. Pin: `X86FloatingPoint.cpp` at d1106deb71cc.

## What is true today

The RFP route is the default: the old assigner that pinned x87 values to FP0..FP5 is gone
([row61-deleted-tests](row61-deleted-tests.md)), greedy allocates the values from the RFP class across block
edges, and `FloatAlloc` runs after `RegAlloc` (`flow.rs`). `fpstack::converted` is the translation of
`X86FloatingPoint`. So this row is not a switch-over: it is the cost of that pass, plus the legalization
that the pin does in selection.

## Cost (d_faces -O2 m32, callgrind inclusive, dist 4700242708; pin: `X86FPStackifierLegacy` 1.7 M)

| Piece | M Ir | Pin counterpart |
|---|---|---|
| `FloatAlloc` total | 22.3 | `X86FPStackifier` 1.7 |
| of it `fpstack::converted` (7 bodies) | 16.2 | `FPS::runOnFunction` |
| of it `fpcopyprop::propagated` | 1.8 | none |
| `FloatPrep` (`floatassign::prepared`) | 8.2 | none: selection does it |

## Steps (one PR each)

| # | Step | Removes (upper bound) | Date |
|---|---|---|---|
| 1 | Callee view of `fpstack::converted` on the ledger command; fix the top cost (per-block rebuilds, liveness) | up to 14 M | PR 11-06 |
| 2 | Retire `fpcopyprop` if the pair shows the stackifier alone leaves no copies the pin's does not | 1.8 M | 11-10 |
| 3 | `FloatPrep` into selection: `fild` from a register via a cell, float-to-int into an owned cell, `_aliased` load reuse become DAG patterns (sdag-b's `FILD`, `FP_TO_INT_IN_MEM`); delete `FloatPrep` | 8.2 M | after sdag-d's emitter 11-18, code 11-20 |
| 4 | One home for the pass: `floatalloc.rs` keeps only what `fpstack.rs` does not | 0 (clarity) | 11-20 |

The saving is 24 M if step 1 reaches the pin's order of magnitude; the pin's 1.7 M is the floor, not the target of
step 1. Allocator-side: with the old pinning gone, the pinned-vreg excess splitkit once counted does not exist; it
is measured only by the d_faces before/after of each PR.

Inputs agreed: sdag-d keeps the unnamed `FloatLoad` for a call result and `FloatStore` for a float return, width 10
with the cell carrying 4 or 8, and leaves `fnstsw; sahf` to the stackifier; sdag-b's contract is
[selectiondag-x87-contract](selectiondag-x87-contract.md).
