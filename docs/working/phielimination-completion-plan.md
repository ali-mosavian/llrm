# PHIElimination: completing the translation (row 61)

`phielimination::lowered` translates `LowerPHINode`, `SplitPHIEdges`, `LoweredPHIs` and the plain `findPHICopyInsertPoint` ([phi-pre-ra-translation.md](phi-pre-ra-translation.md)). What it leaves goes to the old `phielim::eliminated_priced`. Two cases leave phis; both are checked in the code, not assumed.

## What reaches the old lowering today

| case | where | when |
|---|---|---|
| a phi with an incoming edge from a block not in the body (P1) | `phielimination.rs` `phi.incoming.iter().any(|(pre, _)| !at_of.contains_key(pre))` | after a block is deleted and a phi keeps the dead edge: LLVM has no such phi (its phis list only blocks of the function) |
| an x87 phi (P4) | `x87.contains(&phi.result)` | only when `allocate::rfp_on()` is false, i.e. `LLRM_NO_RFP`: RFP is on by default, and under it x87 phis are lowered like any other |

Test-only entries: `phielim::with_old_lowering` (`OLD_LOWERING`), used by `regalloc_total_tests` (2) and `allocate.rs` (1); and `LLRM_NO_PHIELIM_LLVM`.

## Plan

1. P1 is a normalisation, not a lowering: the incoming pair from a block that is not in the body is dropped before lowering (an unreachable edge carries no value). `PHIElimination` then lowers every phi it is given. Test: a phi with a dead edge lowers to the same copies as one without (fails first: it is left a phi).
2. The three tests that ask the old lowering for a large parallel copy are re-expressed on a hand-built allocator input (the property they guard is the allocator's and `ParallelCopy`'s, not the lowering's); then `with_old_lowering`, `OLD_LOWERING` and `LLRM_NO_PHIELIM_LLVM` go.
3. P4 leaves with the non-RFP route (`floatassign` deleted, `LLRM_NO_RFP` gone): with it `eliminated_priced` and `phielim.rs`'s lowering are deleted. Until then it is the one DEVIATE the table row keeps.
4. Each step: objects compared on rows and QCport, wrong-output guards, compile Ir.

Dates: note 2026-11-06; steps 1 and 2 by 2026-11-13; step 3 with the `floatassign` row (owner: its row). Owner w-1.
