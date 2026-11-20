> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/breakfalsedeps.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/breakfalsedeps.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/breakfalsedeps.rs sha256:0e4dc721f1d7dac268a9588c50a5ee3f9fcd49507ca56abc8e93434863879187 -->

# breakfalsedeps

A scalar SSE instruction that writes only the low lane of its destination
waits for the last write of the whole register, unless the register was
last written long enough ago: `BreakFalseDeps`. Translated from
[BreakFalseDeps.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BreakFalseDeps.cpp)
(`processDefs`, the partial-register half; `shouldBreakDependence`) with the
clearance of
[ReachingDefAnalysis.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ReachingDefAnalysis.cpp)
(`getClearance`) and the x86 hooks
[`hasPartialRegUpdate`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86InstrInfo.cpp#L6672),
`getPartialRegUpdateClearance` (L7056) and `breakPartialRegDependency`
(L7453).

## How it works
`cvtsi2ss xmm0, eax` keeps the upper lanes of `xmm0`, so it reads `xmm0`
though its text does not. When the last instruction to write `xmm0` is
fewer than 64 instructions before it, an `xorps xmm0, xmm0` (a zeroing
idiom the core does not wait on) goes just before it. The distance is the
reaching definition's: a definition from a predecessor block is counted
from the end of that block and the nearest of the predecessors' is taken; a
register nothing in the function wrote has no definition and is clear. An
instruction that also reads its destination (`cvtss2sd xmm0, xmm0`) is
left, the dependence is a true one.

```text
  cvtsi2ss xmm0, eax            cvtsi2ss xmm0, eax
  addss    xmm1, xmm0           addss    xmm1, xmm0
  cvtsi2ss xmm0, ecx    ==>     xorps    xmm0, xmm0
                                cvtsi2ss xmm0, ecx
```

## Example
The unit tests `a_convert_right_after_a_write_of_its_register_gets_a_xorps`
and `a_convert_the_register_was_last_written_far_before_is_left`.

## Runs
Last, optimizing levels only: the pin runs it before emission, after block
placement, as the distances are counted in the final order.

## Limits
Only the instructions of the SSE baseline: the pin's AVX forms
(`v`-prefixed, `vxorps`) and the undefined-source half
(`pickBestRegisterForUndef`, the shuffles) have no counterpart here, the
target has no AVX. A call's clobbers are not definitions, as the pin's
register mask is not an operand a def walks. The reaching definitions are
solved to a fixed point over the blocks where the pin walks a loop's blocks
twice; the two agree unless loops nest three deep with the definition in the
innermost. A function with `minsize` gets none (the pin's `hasMinSize`
bail), `optsize` does.
