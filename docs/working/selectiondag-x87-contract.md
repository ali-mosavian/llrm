# x87 contract: legalization → emitter → stackifier (SD6)

Owner sdag-b. Readers: sdag-d (emitter), sdag-c (matcher), w-2 (`X86FPStackifier`). Text mirrors `x86_lowering.rs`, `legalize_dag.rs`, `x86_preprocess.rs`; change both together.

## What the legalizer hands selection (m32, no SSE)

| Source | After legalization |
|---|---|
| `ConstantFP` 0.0, 1.0 (and their negatives) | stays `ConstantFP` (`fldz`, `fld1`, `+chs`): matcher |
| other `ConstantFP` | `Load(Wrapper(TargetConstantPool))`, `MemBase::ConstantPool(entry)`, `dag.constant_pool[entry] = (bits, vt)`; a double exactly a float is a float entry, loaded with `mem_vt` f32 into an f64 result |
| float `Load`/`Store` | plain, `mem_vt` f32/f64/f80; truncating store / widening load where the widths differ |
| `fp_round` (f64→f32, f80→f64/f32) | store to a stack temp of the narrow size, load back (`PreprocessISelDAG`); `fp_round x, 1` is a no-op |
| `fp_extend` | stays; selection picks widening loads/register copies |
| float → int | `X86ISD::FP_TO_INT_IN_MEM chain, slot, value` (mem_vt = result size: i16/i32/i64), then an int `Load` of `slot`. Unsigned i32 uses an i64 slot; unsigned i64 compares with 2^63 (pool) and flips the sign bit of the result. |
| int → float | int stored to a slot, `X86ISD::FILD chain, slot` (mem_vt = int size). Unsigned i32: stored with a zero high dword, filded as i64. Unsigned i64: filded signed, `+2^64` (pool) when the sign was set. |
| compare | `X86ISD::FCMP a, b -> i32` (EFLAGS). `olt/ole/ugt/uge` swap the operands; codes then `A/AE/B/BE/E/NE/P/NP`. `oeq` = `E` and `NP` (two setcc anded), `une` = `NE` or `P`; a branch on them is two `BRCOND`s. |
| select on float | `X86ISD::CMOV` on f32/f64/f80 (flags from an int `CMP` or `FCMP`) |
| `fadd fsub fmul fdiv fneg fabs fsqrt` | unchanged nodes; `frem` is a libcall; `fcopysign` expands; `undef`/`poison` float = `ConstantFP 0` |
| call result | `POP_FROM_X87_REG` after the call (builder); arguments are stack stores |

Every float value is a vreg of class RFP32/64/80 from selection on; stack positions are not decided before the stackifier.

## What selection/emission must do (the pin's own duties, not the legalizer's)

- `FP_TO_INT_IN_MEM` expands as the pin's custom inserter: `fnstcw`, set the rounding bits to truncate, `fldcw`, `fist`, `fldcw` of the saved word.
- `CMOV` on RFP becomes a branch diamond (`EmitLoweredSelect`) on a 486; a P6 `fcmov` is out of scope (target is i486/P5).
- `FCMP` is `fucom` + `fnstsw ax` + `sahf` on i486/P5 (m16 uses the same, 16-bit forms).

## What the stackifier may rely on (w-2)

- At selection: `*_Fp` pseudos on RFP vregs, one def per value; cross-block values are RFP vregs through phis/copies. The stackifier itself runs after allocation on the physical FP0..FP6 and does not rely on one def per value (w-2).
- Float arguments are never in x87 registers; the only x87 values crossing a call boundary are the call result (ST0) and the return value.
- Calls, returns, block edges and inline asm follow the pin's `X86FloatingPoint` (stack empty at calls/returns except ST0; asm clobbers all of FP0..FP6). The legalizer adds nothing there.

## Confirmed by sdag-d and sdag-c (2026-10-23)

1. Pseudo names are whatever sdag-c's pin tables select (`LD_Fp*`, `ST_Fp*`, `ILD_Fp*`, `CHS_Fp`, the `FIST` pseudos); the emitter maps them, no renames.
2. The `fnstcw`/`fldcw` dance around `FP_TO_INT*_IN_MEM` is the emitter's (the pin's custom inserter).
3. f80 spills are `ST_FpP80m` / `LD_Fp80m`, as the pin.
4. sdag-c: `FCMP` is `UCOM_Fpr32/64/80` + `FNSTSW16r` + an `AH` copy + `SAHF` (no `fcomi` on any CPU here); `ConstantFP` f80 0.0/1.0 match `fldz`/`fld1` (#4093); `LLRM_FARCALL` passes through the matcher (#4094).
