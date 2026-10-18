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

1. Done (feat/tti): the trait (`llrm_mir::tti`) and the X86 scalar profile (`llrm_x86::tti`): arithmetic, casts,
   compare and select, loads and stores, `phi`, constants, `isTruncateFree`, `isLegalAddressingMode`.
2. InlineCost reads it behind its switch (`free_instruction`, `gep_free`, `case_clusters` move onto it).
3. Loop strength reduction's two queries (`getScalingFactorCost`, `isLSRCostLess`); the `Machine` hook that hands a
   reader the profile of the compile's `-march`; the generic (no-triple) profile.

Each step: permalink per function, numbered DEVIATE, a test per cost against the pinned `opt` (`print<cost-model>`, at least `-mtriple=i486 -mcpu=i486` and
`-mcpu=pentiumpro`), pinned version in every output header.

## DEVIATE

Each rests on a fact of llrm; the LLVM code it leaves out is linked at d1106deb71cc.

- T1 No vector, masked, gather, interleave or reduction costs. Fact: `Type::Vector` exists in `llrm-mir/src/types.rs`,
  but no frontend and no pass creates one, so no reader asks the price of one. LLVM:
  [`getArithmeticInstrCost` vector tables](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L255) (L255-L1950).
- T2 No per-function target features. Fact: llrm states the CPU once per compile with `-march`; the m32 targets accept
  `i486` and `pentium`, the m16 target `i386 i486 pentium pentiumpro k5 k6 athlon core2` (`timings.times`, `march`
  row of each). LLVM: [`X86TTIImpl(TM, F)` takes the subtarget of each function](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.h#L40).
- T3 The SSE tables are read for `core2` only. Fact: `core2` is the one CPU of those lists with SSE, and neither back
  end has an `xmm` register (`registers.regs` of m16 and m32), so scalar floats are x87 whatever the CPU says. LLVM:
  [`SSE2CostTable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1776), [`SSE1CostTable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1846),
  [`SSE2ConversionTbl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L3526).
- T4 No instruction context in a query. Fact: a reader asks the price of an operation before the instruction exists
  (the SCEV expander) or for a callee's instruction outside its function (InlineCost). LLVM:
  [the `zext` folded into a GEP index and the extending load](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L2780), and the store
  with an indexed address of `getMemoryOpCost`.
- T5 The constant costs and the addressing modes are read off the source, not checked by `opt`. Fact: the cost model
  prints no price for them. LLVM: [`getIntImmCost`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L6892),
  [`getIntImmCostInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L6927),
  [`isLegalAddressingMode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36621). Their tests quote the source lines and are marked
  unverified by oracle.

## What the oracle found

`opt -passes='print<cost-model>'` gives the same price for every scalar instruction on `i386`, `i486`, `pentium`,
`pentiumpro`, `k6` and `athlon`; only `core2` differs (its SSE tables). So the per-`-march` features of `X86Subtarget`
change a scalar price in LLVM only for SSE: CMOV (from `pentiumpro`) and the multiply and shift latencies are not in
the cost model's scalar rows. Nothing here needs a table per CPU but `core2`'s, and no m32 target reaches it.

## One owner per fact: `OperationCosts` and `Tti`

`Machine::costs()` / `size_costs()` (`OperationCosts`, from `opcosts.txt` and `timings.times`: the 486/P5 clock
model, in clocks or bytes of executed work) is read by the native passes, gcc-shaped or llrm's own (`addresssink`, `argpromotion`, `calleepop`, `fill`, `gvn`,
`hoist`, `indvars`, `inline`, `ipacp`, `loopdistribute`, `loopfuse`, `loopinterchange`, `loopversion`, `lsr`, `memcpyopt`,
`peelsize`, `phiopt`, `profit`, `switchlower`, `unswitch`, `window` and the allocator's spill code). `Tti` (LLVM's per-instruction throughput, latency and size units) is read by
the passes that are translations of LLVM's: InlineCost, the SCEV expander, and loop strength reduction once its
translation lands (`lsr` reads `OperationCosts` until then). A pass reads one:
a translated pass keeps LLVM's thresholds, which are tuned to LLVM's units, so it cannot be fed clocks; a native pass
decides in executed clocks or bytes, which `Tti` does not state. Nothing derives one from the other: `Tti` rows are
copies of LLVM's tables checked against `opt`; `OperationCosts` rows come from the target's data sheets. They stay two
because they price two things.

## What loop strength reduction reads

`LoopStrengthReduce.cpp` asks the target about twenty things (the reader is the translated `lsr`, stage S6 of the SCEV
port; `lsr` today is native and reads `OperationCosts`). Done in `Tti`, read off the source and unit-tested (no oracle:
the cost model has no printer for them): `getIntImmCost` ([L1412](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1412)), `isLegalAddressingMode` ([L1862](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1862)),
`getScalingFactorCost` ([L2027](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L2027)), `isLSRCostLess` ([L1663](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1663)), `canMacroFuseCmp` ([L1635](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1635)),
`isLegalICmpImmediate` ([L1897](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1897)), `isLegalAddImmediate` ([L1984](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1984)), `isTruncateFree`
([L4502](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L4502)), `getNumberOfRegisters` ([L1613](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1613)), `isNumRegsMajorCostOfLSR` ([L6385](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L6385), the
base's `true`). X86 answers the base's `false` / none for the rest, so they need no row until a target says otherwise:
`isIndexedLoadLegal` / `isIndexedStoreLegal` ([L1461](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1461), [L3970](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L3970), [L5210](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L5210)),
`isLegalAddScalableImmediate` (L1498), `LSRWithInstrQueries` (L1990), `isProfitableLSRChainElement` (L3161),
`canSaveCmp` (L3586), `shouldDropLSRSolutionIfLessProfitable` (L5694), `getPreferredAddressingMode` (L6315),
`isHardwareLoopProfitable` (L6356), `getTgtMemIntrinsic` (L1053). `isTypeLegal` (L2402) is the legalization of
`tti::Ty`. Not done: the `Machine`-side answer of the 16-bit target (none).
