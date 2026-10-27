# Row 44: MachineSink translation plan

Owner: w-1 from 2026-10-23 (handed over by w-2; PostRAMachineSink and the pressure steps 13/14/15 wait on w-2's row 65 piece 2). Pin d1106deb71cc81016406a1917d0508d2df296266, `llvm/lib/CodeGen/MachineSink.cpp`
([base](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp)).
Ours: `crates/codegen/llrm-codegen/src/backend/machinesink.rs`. Code lands after row 65 piece 3.

Order is the pin's. Status: done, partial, todo, blocked (blocker, date).

| # | Pin function | Line | Ours | Status |
|---|---|---|---|---|
| 1 | `blockPrologueInterferes` | #L337 | none | todo: EH/landing-pad prologue; check LIR has any |
| 2 | `PerformTrivialForwardCoalescing` | #L375 | none | todo: needs copy-forwarding on LIR copies |
| 3 | `PerformSinkAndFold` | #L407 | none | X86 no-op: only `TII->canFoldIntoAddrMode` targets (AArch64, RISCV, LoongArch) run it; x86 has no override. Doc note, no code |
| 4 | `AllUsesDominatedByBlock` | #L649 | `all_uses_dominated_by` | done |
| 5 | `mayLoadFromGOTOrConstantPool` | #L709 | none | todo, with 6 |
| 6 | `FindCycleSinkCandidates` | #L725 | none | deferred: `SinkInstsIntoCycle` is `cl::init(false)` (#L76) and no target enables it; needs 13/14 (row 65 piece 2) and `hasLowDefLatency`. Land after 13/14, flag off |
| 7 | `run` | #L840 | `sunk` | done: block frequencies order the successors (`frequency`), the SinkLoad thresholds (#L91/#L97) are in `has_store_between`; `onEdgeSplit` n/a (the frequencies are made again each round) |
| 8 | `ProcessBlock` | #L959 | `process_block` | partial: reverse walk done, cycle candidates missing |
| 9 | `ProcessDbgInst` | #L1022 | none | n/a: LIR has no debug instructions; note only |
| 10 | `isWorthBreakingCriticalEdge` | #L1040 | `worth_breaking_critical_edge` | done: `SplitEdgeProbabilityThreshold` from `edge_probabilities`; DEVIATE M5 in size builds |
| 11 | `isLegalToBreakCriticalEdge` | #L1111 | `legal_to_break_critical_edge` | done |
| 12 | `PostponeSplitCriticalEdge` | #L1175 | `postpone_split_critical_edge` | done |
| 13 | `registerPressureSetExceedsLimit` | #L1242 | `block_pressure` | done 10-25: one limit (allocatable registers), not per set |
| 14 | `registerPressureExceedsLimit` | #L1256 | none | blocked: same |
| 15 | `isProfitableToSinkTo` | #L1270 | `profitable_to_sink_to` | done 10-25: M2 retired, the tail asks the block pressure |
| 16 | `FindSuccToSinkTo` | #L1413 | `find_succ_to_sink_to` | done |
| 17 | `SinkingPreventsImplicitNullCheck` | #L1521 | none | todo: needs implicit-null-check marking; absent on LIR, note only if no ImplicitNullChecks pass lands |
| 18 | `attemptDebugCopyProp` | #L1566 | none | n/a: debug info |
| 19 | `performSink` | #L1618 | in `sink_instruction` | done |
| 20 | `hasStoreBetween` | #L1660 | `safe_to_move` | done but `mayAlias(AA, MI)` (M7: every store, call and ordered access counts; needs memory operands with an alias query on LIR); the per-block cache and thresholds are in |
| 21 | `aggressivelySinkIntoCycle` | #L1754 | none | deferred with 6; DEVIATE M3 today |
| 22 | `SinkInstruction` | #L1845 | `sink_instruction` | partial: M1, M4 (size rule), M6 |
| 23 | `SalvageUnsunkDebugUsersOfCopy` | #L2005 | none | n/a: debug info |
| 24 | PostRA: `tryToSinkCopy`, `run`, helpers | #L2127-#L2396 | none | todo: PostRAMachineSink copy sinking; runs after RA, new pass slot |

## Order of work

1. Done: 7, 10, 20 (see the table). Left with an input: 2 (`PerformTrivialForwardCoalescing`, 5 of 21 copies in d_faces), M7's alias query (needs LIR memory operands with AA).
2. 11-06: 13/14/15 after row 65 piece 2 lands; 6/8/21 cycle sinking.
3. 11-10: 24 PostRAMachineSink.
4. Each step: a source test, a premise assertion, and the DEVIATE it retires (M1, M2, M3, M5).
