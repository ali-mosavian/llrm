---
architecture-index: TargetTransformInfo: one translation, many readers
architecture-order: 460
---
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

- `llrm_mir::instructioncost::instruction_cost(tti, context, layout, globals, function, inst, kind)`: `getInstructionCost`, the dispatcher (loop rotate, unroll, InlineCost, peel); checked against `print<cost-model>` on `instructioncost_corpus.ll`, 3,416 rows (7 cpus, 4 kinds).
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
- T6 `addresssink` has no price gate on the sink of a masked compare. Fact: x86 priced `test` under the pair on every
  CPU, so no object changed (1,587 compared: vsgcc, QCport, bench bas/nib; 7,547 queries reached the hook). LLVM:
  [`sinkAndCmp0Expression`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2341-L2395)
  asks `isMaskAndCmp0FoldingBeneficial` alone.

## What the oracle found

`opt -passes='print<cost-model>'` gives the same price for every scalar instruction on `i386`, `i486`, `pentium`,
`pentiumpro`, `k6` and `athlon`; only `core2` differs (its SSE tables). So the per-`-march` features of `X86Subtarget`
change a scalar price in LLVM only for SSE: CMOV (from `pentiumpro`) and the multiply and shift latencies are not in
the cost model's scalar rows. Nothing here needs a table per CPU but `core2`'s, and no m32 target reaches it.

TLI hooks are not `Tti` queries. LLVM answers them from `TargetLowering` (`isMaskAndCmp0FoldingBeneficial`, the
`MaxStoresPer*` values); here `llrm_mir::lowering::Lowering` carries them, `Machine::lowering()` hands it out and
`llrm_x86::lowering::X86` answers for every x86 target, whatever its word size. `Tti` keeps the cost queries only, and
the 16-bit target may stay without one. First user: `addresssink` (`masks_sunk`), which dropped `mask_test` and its
gcc-shaped gate `mask_test < 2 * add` (T6).

## Migration owed: MIR passes read `Tti`, LIR keeps `OperationCosts`

The direction (oversight): LLVM has two cost systems, the TTI for IR passes and the scheduling model for machine
passes; llrm's `OperationCosts` is the second. MIR passes read `Tti`; the LIR and the back end keep `OperationCosts`.
Nothing is migrated yet except InlineCost, behind `LLRM_EXP_INLINEDECIDE`. The readers today (24 MIR files and 6 of the back
end and front ends; classification confirmed by oversight, with three corrections below):

Reads `Tti` after migration (their decisions are IR-level and have an LLVM counterpart that asks the TTI):
`gvn`, `hoist`, `indvars`, `inline` (the native inliner, until INLINEDECIDE replaces it), `interprocedural`, `ipacp`
(`getInlineCallPenalty`-like call pricing), `argpromotion`, `candidate`, `loopdistribute`, `loopfuse`, `loopinterchange`,
`loopversion`, `lsr` (once translated), `memcpyopt`, `peelsize`, `phiopt`, `profit`, `switchlower`, `unswitch`, and `addresssink`: LLVM decides addressing forms in CodeGenPrepare through
`isLegalAddressingMode` and `getScalingFactorCost`, so it moves with its TLI hook.

Stays on `OperationCosts` (the price is a machine form or a convention, which LLVM keeps in the target's lowering and
scheduling model, not in the TTI): `calleepop` (the bytes of `ret N` against `add sp`), `window` (a huge pointer's
carry), and the back end and front ends: `cpu`, `loopslots`, `ssaspill`, the model passes (`model/passes.rs`), `abi/qb`
and the `cli` of the front ends that name the CPU.

`fill` mirrors the expansion limits of X86's `TargetLowering`, not gcc's by-pieces: `MaxStoresPerMemset` 16 (8 at
optsize), `MaxStoresPerMemcpy` 8 (4), `MaxStoresPerMemmove` 8 (4), `MaxLoadsPerMemcmp` 2
([X86ISelLowering.cpp:2936-2947](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86ISelLowering.cpp#L2936-L2947)). Keeping gcc's limits is a DEVIATE with a number; otherwise `fill`
translates these values.

`spill` is the register-pressure forecast, which the LICM translation removes: it is deleted with its last reader, not a
permanent stay.

Each migration is its own PR with the same discipline as InlineCost: the named reader, the queries it reads, the rows
with the premise count (queries that reached the TTI) beside the rows line, and the compile-time numbers.

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
because they price two things; the migration owed above moves the MIR readers to `Tti`.
- T7 Intrinsics are priced from a table, not derived. Fact: `BasicTTIImplBase::getIntrinsicInstrCost` and X86's tables
  expand each intrinsic into primitive costs (about 700 lines for the scalar cases), and llrm has 32 intrinsics, so
  `llrm-x86/src/tti_intrinsics.txt` holds the pinned `opt`'s answer per profile, kind and callee name (made by
  `instructioncost_oracle.py`, equal across the cpus without SSE); an intrinsic not in it is `Invalid`. LLVM:
  [`getIntrinsicInstrCost`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1716).
- T8 A GEP is priced without its access type. Fact: X86's `isLegalAddressingMode` reads the scalar type of no mode
  (llrm models no vector). LLVM: [`getGEPCost`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L1348).
- T9 `fpround` (llrm's own instruction: a float rounded to its type's precision) costs `TCC_Basic`, the default of the
  switch; LLVM has no such instruction.
- T10 A target with no cost model is priced by `Basic`. Fact: `Machine::tti` is None for a target that has none, and a pass
  names no target crate. LLVM: [`TargetTransformInfoImplBase`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h).
- T11 `LoopMicroOpBufferSize` is 0. Fact: the scheduling models of the pinned cpus (`i486`, `pentium`, `pentiumpro`) have none, so
  `BasicTTIImpl::getUnrollingPreferences` sets nothing. LLVM: [`getUnrollingPreferences`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L598).
