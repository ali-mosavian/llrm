# The d_faces m32 -O2 gap, like for like (w-1, 2026-10-27)

Owner: w-1. Callgrind Ir inclusive of w5's dist (`feat/parity` f48ca0873, 742.2 M) against the pin's clang d1106deb71cc (508.1 M), `tools/ledger/ledger4.py` and
`passtable.py` on the two profiles (`docs/working/ledger-command.md`).

The pin's `X86DAGToDAGISel::runOnMachineFunction` includes the DAG build, combines, legalization and selection; ours is `isel` alone, and the work
of the pin's DAG combines, `X86FixupBWInsts` and `PeepholeOptimizer` is done in our `Peephole` and its small neighbours. Read as two buckets,
isel is -18.0 M and clean-up +75.2 M; they are one bucket.

| group | ours M | pin M | excess M | owner |
|---|--:|--:|--:|---|
| isel + combine + fixups (ours: isel 36.4, Peephole 37.0, peepholeopt 1.1, fixupbw 1.1, pushfold 0.8, cmpsub 0.3; pin: DAGToDAGISel 54.4, PeepholeOptimizer + FixupBW 2.1) | 76.7 | 56.5 | +20.2 | w-1 |
| machine clean-up, the rest | 70.1 | 47.4 | +22.7 | below |
| regalloc (RA, coalesce, phi-elim, two-address, parcopy, and the float allocation of floatalloc and floatassign, 14.3) | 129.1 | 67.4 | +61.7 | w-13 |
| loop passes | 112.7 | 87.1 | +25.6 | w-5 |
| SimplifyCFG, JumpThreading, CVP | 62.8 | 33.6 | +29.2 | unassigned |
| residual (pass-manager glue, analyses outside passes, allocation) | 64.6 | 20.9 | +43.7 | unassigned |
| front end | 49.1 | 41.1 | +8.0 | unassigned |
| emit | 11.6 | 4.1 | +7.5 | unassigned |
| scalar (GVN, EarlyCSE, SROA, ...) | 88.0 | 81.7 | +6.3 | unassigned |
| InstCombine, InstSimplify, Reassociate | 61.3 | 55.1 | +6.2 | unassigned |
| llrm-only MIR passes, IPO | 16.3 | 13.3 | +3.0 | unassigned |
| total | 742.2 | 508.1 | +234.1 | |

The isel group is one row, +20.2 M, where the table read +75.2 and -18.0 apart. The float allocation of `floatalloc`/`floatassign` (valued, split_for_the_stack, live rows, live_among; w-2's split, ~14.3 M) has the pin's counterpart inside RAGreedy, so it counts with regalloc: the clean-up excess is +22.7, not +75.2.

## Machine clean-up, the rest (excess M, ours M / pin M)

The FP stackifier is alone here (3.0 against the pin's 1.7); the 14.3 M of float allocation is in the regalloc row above.

| pass | excess | ours / pin | owner |
|---|--:|---|---|
| FP stackifier proper (the `Fps` handlers and the stack conversion) | +1.3 | 3.0 / 1.7 | w-2 |
| jumps::ControlFlow (BranchFolder, block placement) | +9.8 | 15.7 / 5.9 | w-3 |
| MachineScheduler | +5.3 | 15.7 / 10.4 | w-2 |
| MachineSink | +4.5 | 9.0 / 4.5 | w-2 |
| MachineCSE | +3.0 | 5.9 / 2.9 | w-2 |
| LateCleanup | +2.2 | 3.4 / 1.2 | funcattrs-port |
| fixupleas, leaopt, fixupsetcc, flagscopy, callframe | +3.6 | 3.6 / 0.0 | w-2 |
| phiconst, loopslots, liverangeshrink, earlytaildup, combiner | +6.7 | 8.0 / 1.3 (pin: LRS) | w-3 |
| MachineLICM | -5.7 | 3.4 / 9.1 | |
| MachineCP | -1.1 | 2.3 / 3.4 | |
| pin only (ReachingDef 3.3, PEI 1.0, ShrinkWrap 0.3, ExecutionDomainFix 0.9, DeadMachineInstructionElim 1.4) | -6.9 | 0 / 6.9 | |

Rows are inclusive and the groups do not overlap (the check of `ledger4.py` ran without `LEDGER_CALLING`: not run). Peephole is flat (see `cleanup-bucket-by-pass.md`).
