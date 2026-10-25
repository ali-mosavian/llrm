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
