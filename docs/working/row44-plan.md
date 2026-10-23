# Row 44: MachineSink translation plan

Owner: w-1 from 2026-10-23 (handed over by w-2; PostRAMachineSink and the pressure steps 13/14/15 wait on w-2's row 65 piece 2). Pin d1106deb71cc81016406a1917d0508d2df296266, `llvm/lib/CodeGen/MachineSink.cpp`
([base](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp)).
Ours: `crates/backend/llrm-core/src/backend/machinesink.rs`. Code lands after row 65 piece 3.

Order is the pin's. Status: done, partial, todo, blocked (blocker, date).

| # | Pin function | Line | Ours | Status |
|---|---|---|---|---|
| 1 | `blockPrologueInterferes` | #L337 | none | todo: EH/landing-pad prologue; check LIR has any |
| 2 | `PerformTrivialForwardCoalescing` | #L375 | none | todo: needs copy-forwarding on LIR copies |
| 3 | `PerformSinkAndFold` | #L407 | none | X86 no-op: only `TII->canFoldIntoAddrMode` targets (AArch64, RISCV, LoongArch) run it; x86 has no override. Doc note, no code |
| 4 | `AllUsesDominatedByBlock` | #L649 | `all_uses_dominated_by` | done |
| 5 | `mayLoadFromGOTOrConstantPool` | #L709 | none | todo, with 6 |
| 6 | `FindCycleSinkCandidates` | #L725 | none | deferred: `SinkInstsIntoCycle` is `cl::init(false)` (#L76) and no target enables it; needs 13/14 (row 65 piece 2) and `hasLowDefLatency`. Land after 13/14, flag off |
| 7 | `run` | #L840 | `sunk` | partial: no `UseBlockFreqInfo` path (#L77), no SinkLoad thresholds (#L91/#L97) |
| 8 | `ProcessBlock` | #L959 | `process_block` | partial: reverse walk done, cycle candidates missing |
| 9 | `ProcessDbgInst` | #L1022 | none | n/a: LIR has no debug instructions; note only |
| 10 | `isWorthBreakingCriticalEdge` | #L1040 | `worth_breaking_critical_edge` | blocked: `SplitEdgeProbabilityThreshold` (#L82) needs `MachineBranchProbabilityInfo::of(&LirBody).get_edge_probability(block, succ_index)` (w-5, accessor done in #3845); MBFI exists (`liveness::blockfrequency::MachineBlockFrequencyInfo`); DEVIATE M5 in size builds |
| 11 | `isLegalToBreakCriticalEdge` | #L1111 | `legal_to_break_critical_edge` | done |
| 12 | `PostponeSplitCriticalEdge` | #L1175 | `postpone_split_critical_edge` | done |
| 13 | `registerPressureSetExceedsLimit` | #L1242 | none | blocked: region register pressure sets = row 65 piece 2 (w-2, 11-06); M2 stands until then |
| 14 | `registerPressureExceedsLimit` | #L1256 | none | blocked: same |
| 15 | `isProfitableToSinkTo` | #L1270 | `profitable_to_sink_to` | partial: M2 ends it with a pressure check; lifted by 13/14 |
| 16 | `FindSuccToSinkTo` | #L1413 | `find_succ_to_sink_to` | done |
| 17 | `SinkingPreventsImplicitNullCheck` | #L1521 | none | todo: needs implicit-null-check marking; absent on LIR, note only if no ImplicitNullChecks pass lands |
| 18 | `attemptDebugCopyProp` | #L1566 | none | n/a: debug info |
| 19 | `performSink` | #L1618 | in `sink_instruction` | done |
| 20 | `hasStoreBetween` | #L1660 | `safe_to_move` | partial: ours is whole-path; the pin's per-block cache and thresholds (#L1704) missing; M1 |
| 21 | `aggressivelySinkIntoCycle` | #L1754 | none | deferred with 6; DEVIATE M3 today |
| 22 | `SinkInstruction` | #L1845 | `sink_instruction` | partial: M1, M4 (size rule), M6 |
| 23 | `SalvageUnsunkDebugUsersOfCopy` | #L2005 | none | n/a: debug info |
| 24 | PostRA: `tryToSinkCopy`, `run`, helpers | #L2127-#L2396 | none | todo: PostRAMachineSink copy sinking; runs after RA, new pass slot |

## Order of work

1. 11-02: 20 per-block store cache and thresholds; 7/8 SinkLoad thresholds; 1, 2, 5 where LIR has the input.
2. 11-06: 13/14/15 after row 65 piece 2 lands; 6/8/21 cycle sinking.
3. 11-10: 24 PostRAMachineSink; 10 once #3845 lands; the `UseBlockFreqInfo` path (#L77) can start now on the existing MBFI.
4. Each step: a source test, a premise assertion, and the DEVIATE it retires (M1, M2, M3, M5).
