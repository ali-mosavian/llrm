> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_demanded.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_demanded.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_demanded.rs sha256:775e1c2e2c877978fe6cbe1dd985c31265ef65a7cd58b65e282a0d30429ba699 -->

# combine_demanded

`SimplifyDemandedUseBits`: an integer instruction is rewritten, or its
operands are, when the users read only some of its bits.

Translation of
[`InstCombinerImpl::SimplifyDemandedUseBits`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineSimplifyDemanded.cpp#L235),
`SimplifyDemandedBits` (:166), `SimplifyDemandedInstructionBits` (:132),
`SimplifyMultipleUseDemandedBits` (:1228), `simplifyShrShlDemandedBits`
(:1404), `simplifyShiftSelectingPackedElement` (:70) and
`ShrinkDemandedConstant` (:43) at d1106deb71cc.

MIRROR: the order of the cases and of the checks in each, the demanded mask
each operand is asked with, the flags dropped after an operand changed, and
the known bits each case leaves for its caller.

DEVIATE 1: `ctpop` and `ptrmask` are not rewritten. `(ctpop X) ^ (ctpop Y)`
and `ctpop (not X)` build an intrinsic call (`combine_build` DEVIATE 2), and
`ptrmask` reads a pointer's known bits; their calls get the known bits of
`computeKnownBits`, as LLVM's fall-through does.

DEVIATE 2: `convertOrOfShiftsToFunnelShift` is asked only as "is this `or` a
rotate": the shift pair, not the `zext` concatenation (:3114). A
`shl`/`lshr` that feeds only a concatenation is simplified here where LLVM
leaves it.

DEVIATE 3: `haveNoCommonBitsSet` of the cached known bits asks the
operands' known bits again as well; the answer is only ever "yes" more
often when the fresh bits know more than the cached.

DEVIATE 4: `m_ImmConstant` is an integer constant; `undef` and `poison` do
not match. No vectors (MIR has none), no target-specific hook.

## Used by
The `visit*` of the integer families (`visitAnd`, `visitAdd`, `visitTrunc`,
`visitSelect`...) through `Combiner::simplify_demanded_instruction_bits`.

## Example
```text
%a = or i32 %x, 255
%b = and i32 %a, 65280      ; only bits 8..15 are read, so %a is %x
```
