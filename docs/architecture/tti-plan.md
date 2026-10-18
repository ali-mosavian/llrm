# TargetTransformInfo: one translation, many readers

TL;DR: translate `TargetTransformInfo` with `BasicTTIImpl` (the generic profile `opt` uses without a triple)
and `X86TTIImpl` (the i486 scalar subset) once, in `llrm-mir` (the interface) and `llrm-x86` (the profile), for InlineCost, the SCEV expander, lsr, unroll
and loop-idiom. Owner: cgscc-poc. Source: llvm-project at d1106deb71cc.

## Source map (llvm/)

| Unit | C++ | Lines |
|---|---|---|
| Interface and defaults | `include/llvm/Analysis/TargetTransformInfo.h`, `TargetTransformInfoImpl.h`, `lib/Analysis/TargetTransformInfo.cpp` | 2,246 / 1,790 / 1,642 |
| Generic profile | `include/llvm/CodeGen/BasicTTIImpl.h` | 3,616 |
| X86 profile | `lib/Target/X86/X86TargetTransformInfo.cpp` | 8,210 |
| Legal addressing | `X86TargetLowering::isLegalAddressingMode` (`X86ISelLowering.cpp`) | |

Only scalar integer and x87 paths are translated, for each supported `-march` (i386, i486, pentium, pentiumpro, k5, k6,
athlon, core2): the scalar subtarget features of `X86Subtarget` (CMOV from pentiumpro, per-CPU multiply and shift
costs). The vector, masked, gather, interleave and reduction costs return `Invalid` (DEVIATE T1: no producer).

## Interface (Rust: `llrm_mir::tti` is the trait, `llrm_x86::tti::X86` the profile)

`Cost { value: i64, valid: bool }` is `InstructionCost`. `CostKind` is `TCK_RecipThroughput | Latency | CodeSize |
SizeAndLatency`. One trait, `Tti`, with a profile each (`Generic`, `X86 { cpu }`, `cpu` from `-march` (its scalar features)):

- `instruction_cost(function, inst, kind)`: `getInstructionCost`, the dispatcher (InlineCost, unroll).
- `arithmetic_cost(opcode, ty, kind, op1_info, op2_info, args)`, `cast_cost(opcode, dst, src, context, kind)`,
  `cmp_sel_cost(opcode, value_ty, cond_ty, predicate, kind)`, `memory_op_cost(opcode, ty, align, space, kind)`,
  `cf_instr_cost(opcode, kind)`, `int_imm_cost(imm, ty)`, `int_imm_cost_inst(opcode, idx, imm, ty, kind)`.
- `is_truncate_free(from, to)`. Constants are passed as `(bits: u128, width: u32)`; the SCEV expander
  (ScalarEvolutionExpander.cpp L1965-2223) reads `is_truncate_free`, `cast_cost`, `arithmetic_cost` (default operand
  infos), `cmp_sel_cost`, `cf_instr_cost(PHI)` and `int_imm_cost_inst`, with kind `RecipThroughput` or `CodeSize`.
- `is_legal_addressing_mode(ty, base_gv, base_offset, has_base_reg, scale, space)`, `scaling_factor_cost(...)`,
  `is_lsr_cost_less(a, b)`, `can_macro_fuse_cmp()`.
- `inline_call_penalty(caller, call)`, `are_inline_compatible(caller, callee)`, `is_lowered_to_call(callee)`.

Readers take `&dyn Tti`; the target decides the profile once per module.

## Plan

1. `Generic` (`BasicTTIImpl` scalar rules) and the trait; move `free_instruction`, `gep_free`, `case_clusters` out of
   `inlinecost.rs` onto it; no output change (#2140's tests).
2. `X86` per `-march`: `getArithmeticInstrCost`, `getCastInstrCost`, `getCmpSelInstrCost`, `getIntImmCost*`,
   `getMemoryOpCost`, `isLegalAddressingMode`, `getScalingFactorCost`, `isLSRCostLess` for i486 (scalar tables
   with their feature gates).
3. InlineCost switches to `X86` behind its switch; the SCEV expander and lsr read the same trait.

Each step: permalink per function, numbered DEVIATE, a test per cost against the pinned `opt` (`print<cost-model>`, at least `-mtriple=i486 -mcpu=i486` and
`-mcpu=pentiumpro`), pinned version in every output header.

## DEVIATE

- T1 No vector, masked, gather, interleave or reduction costs.
- T2 `getInlineCallPenalty` and `areInlineCompatible` read no per-function target-feature attributes (llrm sets features
  per module with `-march`).
- T3 llrm's `OperationCosts` stay what the allocator and `profit` read; they are not TTI.
