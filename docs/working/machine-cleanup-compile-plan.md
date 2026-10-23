# Machine clean-up compile time: plan to 1.0x of the pin

Owner: w-2. Dated 2026-10-23, due 10-26. Bucket "machine clean-up + FP stackifier" of
[ledger4](ledger-command.md): ours 148.9 M, pin 49.4 M, excess 99.4 M (d_faces -O2 m32, dist
`own2-c1ee613cda`, `ledger4.py` on `callgrind_annotate --inclusive=yes`; the coordinator's one-tip
table says 135.1 M, the same bucket at another commit). One PR per step; each states the ledger
command and the before/after M. "Remove" is ours minus the pin counterpart, an upper bound until the
step is measured.

| # | Step (pin function it mirrors) | Ours M | Pin M | Remove M | PR by |
|---|---|---|---|---|---|
| 1 | Delete post-RA `machinecse` (the pin's MachineCSE runs pre-RA only) | 3.3 | 0 | 3.3 | 11-02 |
| 2 | `DeadMachineInstructionElim` for `machinedce` | 2.1 | 1.4 | 0.7 | 11-06 |
| 3 | MachineSink profile and fix: row 44 ([plan](row44-plan.md)) | 10.9 | 4.3 | 6.6 | 11-02 |
| 4 | `PostRAMachineSinking` for `postrasink` (row 44 step 24) | 4.4 | 0.2 | 4.2 | 11-10 |
| 5 | `MachineCopyPropagation` for `copysink` + `spillforward` | 12.0 | 3.4 | 8.6 | 11-13 |
| 6 | `X86OptimizeLEAs` / `X86FixupLEAs` / `convertToThreeAddress` for `addresses` + `lea_fixup` | 10.0 | 0.3 | 9.7 | 11-13 |
| 7 | Row 45 PeepholeOptimizer pre-RA ([plan](row45-plan.md)); retire each post-RA rule group it makes dead, and one window walk for the rest | 41.9 (Peephole 73.7 less steps 1, 2, 4, 5, 6) | 1.0 | 40.9 | 11-06 first piece, rest 11-20 |
| 8 | `BranchFolder` + `MachineBlockPlacement` for `jumps::ControlFlow` (row 82 plan) | 17.6 | 5.9 | 11.7 | 11-13 |
| 9 | `X86FPStackifier` for `FloatAlloc` + `FloatPrep` | 32.8 | 1.7 | 31.1 | plan 11-06, code 11-20 |
| 10 | Retire `LoopSlots`, `SharedConstants` (no pin counterpart) | 5.4 | 0 | 5.4 | 11-13 premise |
| 11 | Row 65 Machine Scheduler replaces `PressureSink` | 4.8 | 10.4 | -5.6 | 11-06 |

The upper bounds sum to 117 M against an excess of 99.4 M (the scheduler adds 5.6 M); the bucket reaches
1.0x if the steps deliver 85% of their bound. Each PR reports what it got.

Split of Peephole (tree view, share of its 96.9 M): addresses 11%, spillforward 9%, copysink 7%,
machinecse 5%, postrasink 6%, machinedce 3%, `narrowed_arithmetic` 5%, `fused` 4%, `high_extracts`
4%, `constants` 3%, `zeroes` 3%, the other rule groups under 3% each.

Measuring: callgrind on the ledger command with a stamped dist of the parity tip contained in the
head; the pin side is the same command with pinned clang. Pin per-pass inclusive M is from that
profile (MachineScheduler 10.4, MachineLICM 9.1, MachineSink 4.3, BlockPlacement 3.9, CopyProp 3.4,
ReachingDef 3.4, MachineCSE 2.9, BranchFolder 2.0, FPStackifier 1.7, DeadMachineInstructionElim 1.4,
LateInstrsCleanup 1.2, Peephole 1.0, FixupBW 1.0, PEI 1.0).
