# Machine clean-up compile time: plan to 1.0x of the pin

Owner: w-2. Dated 2026-10-23, due 10-26. Bucket "machine clean-up + FP stackifier" of
[ledger4](ledger-command.md): ours 157.7 M, pin 49.5 M, excess 108 M (d_faces -O2 m32, dist
`own2-4700242708`, re-measured 2026-10-24 with `ledger4.py` on this profile and the pinned-clang
profile). The first version of this plan quoted 148.9 M / 99.4 M: those came from the ledger
directory's own profile files, which `ledger4.py` reads by fixed name whatever its arguments are,
not from the dist named there. The per-pass numbers below are from the first read; the order of
magnitude holds (Peephole 74.6 M now against 73.7 M, FloatAlloc 22.3 M both times).
One PR per step; each states the ledger
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
| 8 | `BranchFolder` + `MachineBlockPlacement` for `jumps::ControlFlow` (row 82 plan; w-2 owns BranchFolder, cost and completion; tail duplication rows 34/83 and placement row 90 are splitkit's, split agreed in writing) | 17.6 | 5.9 | 11.7 | 11-13 |
| 9 | `X86FPStackifier` cost, see [fp-stackifier-plan](fp-stackifier-plan.md); not a switch-over (the allocator route is the default); `FloatAlloc` + `FloatPrep` (inputs agreed with sdag-d: x87 emitter 11-20, sdag-b: SD6 fpstack contract) | 32.8 | 1.7 | 31.1 | plan 11-06, code 11-20 |
| 10 | Retire `LoopSlots`, `SharedConstants` (no pin counterpart) | 5.4 | 0 | 5.4 | 11-13 premise |
| 11 | Row 65 Machine Scheduler replaces `PressureSink` | 4.8 | 10.4 | -5.6 | 11-06 |

The upper bounds sum to 117 M against an excess of 99.4 M (the scheduler adds 5.6 M); the bucket reaches
1.0x if the steps deliver 85% of their bound. Each PR reports what it got.

Split of Peephole (tree view, share of its 96.9 M): addresses 11%, spillforward 9%, copysink 7%,
machinecse 5%, postrasink 6%, machinedce 3%, `narrowed_arithmetic` 5%, `fused` 4%, `high_extracts`
4%, `constants` 3%, `zeroes` 3%, the other rule groups under 3% each.

Measuring: callgrind on the ledger command with a stamped dist of the parity tip contained in the
head; the pin side is the same command with the pinned clang (`clang` as in the ledger command,
`clang version 24.0.0git d1106deb71cc81016406a1917d0508d2df296266`, `-O2 -m32 -march=i486 -c -o` a real file). Pin per-pass inclusive M is from that
profile (MachineScheduler 10.4, MachineLICM 9.1, MachineSink 4.3, BlockPlacement 3.9, CopyProp 3.4,
ReachingDef 3.4, MachineCSE 2.9, BranchFolder 2.0, FPStackifier 1.7, DeadMachineInstructionElim 1.4,
LateInstrsCleanup 1.2, Peephole 1.0, FixupBW 1.0, PEI 1.0).

## X86FPStackifier inputs (step 9)

From sdag-b (docs/working/selectiondag-plan.md, SD6): FP values are RFP vregs until this pass; selection emits the
pin's `*_Fp` pseudos (X86InstrFPStack.td): `FCMP` to UCOM_Fpr + FNSTSW16r + SAHF (m16 differs in fnstsw/sahf only),
`FILD` from a stack slot, `FP_TO_INT_IN_MEM` as fist to a slot, FP CMOV as `CMOV_RFP32/64/80`, constants as
constant-pool loads, `fp_round` as store and reload, loads and stores as `LD_Fp*m`/`ST_Fp*m`, a call's float
result as `POP_FROM_X87_REG`. Calls, returns, block edges and inline asm follow the pin's X86FloatingPoint (stack
empty at call and ret except the ST0 result, `LiveBundle` layouts at edges). Exact pseudos per instruction:
sdag-c and sdag-d (reply pending).

Allocator (splitkit, from the code at parity): greedy allocates x87 values from the RFP class (`allocate.rs`, seven flat
registers MM0..MM6 for FP0..FP6) with the normal region split, so values cross block edges; the spiller spills at float
width. Nothing is missing there. `floatassign::prepared` (FloatPrep) does three things LLVM does in isel: fild from a
register via a cell, a float-to-int store into an owned cell, and `_aliased` load reuse. They are the input legalization
(sdag-b's `FILD` from a slot and `FP_TO_INT_IN_MEM`), so FloatPrep is deleted when selection emits them; the stackifier
`fpstack.rs` is already the translation of X86FloatingPoint and stays; what goes is `floatalloc.rs` and `floatassign.rs`
(the flat assignment and its own liveness), once selection feeds RFP values to the allocator. Step 9 is that switch-over
plus the deletion; the 32.8 M is those two passes.

## Callee views (d_faces -O2 m32, callgrind inclusive, dist 4700242708, 2026-10-24)

`fpstack::converted` 16.2 M (row 9): `_split_for_the_stack` 2.1, `Ahead::order` 2.0 (cached in #4368: -1.1 M), `fpregs::valued` 1.8, `live_among` x2 3.3, block rebuild 1.0, `spillplacement::bundles` 0.6. No single cost dominates.

`machinesink::sunk` 12.5 M for 7 bodies, 9 rounds (pin: `MachineSinkingLegacy` 4.3 M): `MachineBlockFrequencyInfo::of` 1.7 (9 calls; it builds its own graph and cycle info), `loops::irreducible` 1.4, `liveness::effect` 1.3 (2,328 first-time decodes), the flag-liveness solve 1.3, `loops::loops` 0.7, `find_succ_to_sink_to` 0.3, `DomTree::of` 0.5; the rest, about 4.5 M, is the walk itself. The lever is sharing: the frequency info, the branch-probability info, the sinker's loops and its dominator tree each rebuild the graph and the cycles of the same body. That needs `MachineBlockFrequencyInfo` and `MachineBranchProbabilityInfo` to take a graph and cycle info already built (w-5's module), then one build per body feeds all four: up to 4 M, not measured.

## Re-measured on the dist build (2026-10-25, dist e482d470ae, d_faces -O2 m32, `ledger4.py` per-pass rows; the ledger's front-end and residual rows read other files and are not quoted)

The release profile above overstates the passes by about 15% (Peephole 74.6 M there, 62.9 M on dist). The clean-up bucket on dist: ours 132.6 M, pin 49.5 M, excess 83.2 M. By pass: Peephole 62.9, jumps::ControlFlow 19.6, FloatAlloc 16.3, MachineSink 9.1, FloatPrep 8.7, precse 5.6, pressuresink 4.4, loopslots 3.3, phiconst 1.7, floatfold 0.8.

Landed on dist (same-tree pairs): #4368 -0.81 M, #4395 -1.56 M, #4412 -1.72 M. Row 9 after them: FloatAlloc 16.3 M, `fpstack::converted` 10.8 M. `fpregs::valued` (1.8 M release) is real work, not a lever: 1.1 M of it rewrites the instructions of the 2 bodies that carry x87 operands. What is left in `converted` is spread over the stack model itself (no single item above 1 M), so the remaining gap to the pin's 1.7 M is the pass's design, not a scan that can be cached; the way down is `FloatPrep` into selection (8.7 M, after sdag-d 11-18) and fewer bodies reaching the pass.

