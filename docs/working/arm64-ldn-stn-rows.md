# arm64 ld2/st2/st4 selection (S4): what is left

Piece 1 is in: `VectorLoadStructure` / `VectorStoreStructure` (encode, text, LIR bridge), encodings checked against the pinned `llvm-mc`. It moves no cell.

| Row | Piece | Pin | Owner | Exit |
|---|---|---|---|---|
| LDN-2 | IR pass: a deinterleaving/interleaving shuffle of a wide access becomes `llvm.aarch64.neon.ld2/st2/st4`; multi-result intrinsic path in the DAG | `InterleavedAccessPass.cpp`, `AArch64TargetLowering::lowerInterleavedLoad/Store` | llrm-w11 | 2026-11-24 |
| LDN-3 | Consecutive-register tuples `QQ`/`QQQ`/`QQQQ` (`qsub0..3`): `llrm_lir::registers::Info` models a register as one view of one root, so a tuple, which spans several roots, needs a multi-root register in `Info`, the allocator's overlap test and the greedy's class tables | `AArch64RegisterInfo.td` (`DD`, `QQ`, `QQQ`, `QQQQ`) | llrm-w9-debug-lines | 2026-11-24 |

`Tti::selects_interleaved_accesses` goes true when LDN-2 and LDN-3 land.
