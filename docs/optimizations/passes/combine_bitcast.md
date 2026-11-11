> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_bitcast.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_bitcast.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_bitcast.rs sha256:8a8ca7c1986b8baf050ce865ddfc49e2abef47c22797c58719cc438d0c756f0c -->

# combine_bitcast

InstCombine's `visitBitCast`, from
[`InstCombineCasts.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCasts.cpp#L3426)
at d1106deb71cc: its first step, a cast from a type to itself is its
operand.

The cast-pair table turns `fptrunc (fpext x)` back to `x`'s type into a
`bitcast` to the same type, which this step removes (`commonCastTransforms`
creates it, the next visit folds it).

MIRROR: the order of the steps.

DEVIATE 1: the rest of `visitBitCast` is not translated: the vector steps
(MIR has no vectors), `optimizeBitCastFromPhi`, `canonicalizeBitCastExtElt`,
`foldBitCastBitwiseLogic`, `foldBitCastSelect` and the closing
`commonCastTransforms`. Exit: 2026-11-20.

## Example
```text
%r = bitcast float %x to float           ; %r is %x
```

## Used by
`combine` (its `visit`, the `BitCast` opcode).
