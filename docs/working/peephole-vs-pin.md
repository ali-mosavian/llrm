# Peephole against the pin: where the Ir goes

2026-10-25, d_faces -O2 m32, callgrind. Ours: dist build 898f99e494 (Peephole unchanged since). Pin: `llc -O2 -mtriple=i486 -mcpu=i486` on the clang IR of d_faces (`w2df.ll`), so the inputs differ in IR, not in program.

Ours: Peephole 46.8 M, ControlFlow 18.0, LateCleanup 4.9. The pin's post-RA passes together are about 12 M (MachineCopyPropagation 2.8, ReachingDef 3.2 for BreakFalseDeps/ExecutionDomainFix, BranchFolder 2.0, X86FixupBW 1.1, MachineLateInstrsCleanup 1.0, ExecutionDomainFix 0.8, PostRAMachineSink 0.25).

| Our sub-pass | Ir (M) | Pin counterpart | Pin Ir (M) |
|---|---|---|---|
| `machinecp::propagated` | 7.05 | MachineCopyPropagation | 2.82 |
| generated `narrowed_arithmetic` | 3.47 | isel patterns (no post-RA cost) | 0 |
| `_flags_dead_after` + `_flags_live_out` | 4.3 | none (flags tracked per MI) | 0 |
| `LiveRows::sets` + `live_rows` | 3.1 | none | 0 |
| `postrasink::sunk` | 2.24 | PostRAMachineSinking | 0.25 |
| `peephole::zeroes` | 1.95 | isel (`MOV32r0`) | 0 |
| generated `fused` | 1.92 | isel patterns | 0 |
| register effects table (`OnceLock` fill) | 12.1 inclusive, shared by the above | none | 0 |
| `machinedce::eliminated` | 1.67 | DeadMachineInstructionElim | 1.34 (all runs) |
| `popped_arguments` | 1.38 | FrameLowering/CFO | n/a |
| `copysink::sunk` | 1.16 | none | 0 |

Reading: the structural gap is not one pass. Peephole redoes at post-RA what the pin's isel patterns do before it (narrowing, fusing, zeroes), and pays an effects/flags/liveness table for every query. Cuts that need no new design: the effects table (12.1 M inclusive, `EffectKey::hashes` 4.5), flags liveness (4.3), `machinecp` against MachineCopyPropagation (7.05 vs 2.82), `postrasink` (2.24 vs 0.25).

## Tracked row: PH-ISEL-1, post-RA work the pin does at ISel (2026-10-27)

d_faces -O2 m32, parity 59c834686, callgrind checked with `tools/callgrind-check.py` (0 false edges under `Peephole::once`). `Peephole::once` is 33.7 M Ir for 6 functions: one run per function, not six. The pin's post-RA equivalents are MachineCopyPropagation 3.4 + PeepholeOptimizer 1.0 + BranchFolder 3.0. The gap is content, not pass count: the sub-passes below do at post-RA what the pin does before register allocation. It shrinks as the DAG selector takes each over; each moves out of `once` when its row lands.

Pin homes are from each sub-pass's name and header, not yet checked against the pin source: the owner who picks a row confirms the home first.

| Sub-pass (peephole.rs `once`) | Pin home | Lane |
|---|---|---|
| `extensions`, `zero_extensions`, `widened_moves`, `upperzero::hoisted` | X86 ISel zext/sext patterns, DAGCombiner known bits | DAG matcher, combiner |
| `narrowed_arithmetic`, `narrowed_moves` | ISel width patterns, `reduceLoadWidth` | DAG matcher, combiner |
| `fused`, `fused_calls`, `far_loads` (load folding) | ISel load/op patterns; PeepholeOptimizer `foldMemoryOperand` | DAG matcher |
| `zeroes`, `zeroed_loads` | ISel `MOV32r0` and pseudo expansion | DAG matcher |
| `zero_compares`, `tested`, `compares_again`, `stepped_after_compare` | PeepholeOptimizer `optimizeCmpInstr`; ISel `TEST` patterns | peephole (same place), matcher |
| `addresses`, `_affine_address`, `loaded_addresses` | X86DAGToDAG `matchAddress`; X86FixupLEAs | DAG matcher |
| `borrows`, `increments`, `doubled`, `commuted` | ISel adc/sbb/inc/dec/shl-as-add patterns; TwoAddress commute | DAG matcher, twoaddr |
| `storecombine`, `sharedstores::shared` | DAGCombiner `MergeConsecutiveStores`; MachineCSE | DAG combiner |
| `pushed_constants`, `pushes`, `popped_arguments` | ISel `PUSH32i` patterns; X86CallFrameOptimization | DAG matcher, frame |
| `copysink::sunk`, `postrasink::sunk`, `latched` | MachineSink; PostRAMachineSinking | post-RA, same place |
| `machinecp::propagated`, `forwarded_copies` | MachineCopyPropagation (twice per function, as ours) | same place |
| `machinedce::eliminated` | DeadMachineInstructionElim | same place |
| `slotcopies::merged`, `forwarded_reloads`, `unread_spill_stores` | InlineSpiller, StackSlotColoring | allocator |
| `loaded_segments`, `exact_addresses`, `phielim::unsplit` | none (segments, our phi form) | LLRM-only |

Not a lever: re-scheduling the pass. The count is one run per function, as the pin's.
