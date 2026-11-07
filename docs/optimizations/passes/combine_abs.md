> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_abs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_abs.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_abs.rs sha256:4349af568cd7f7d0b26d192f538538c2433ef2d0b77b7cb3e3c697b70cb3b0d4 -->

# combine_abs

InstCombine's `llvm.abs`: the `Intrinsic::abs` case of
`InstCombinerImpl::visitCallInst`
([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L2177)).

## How it works
The operand's sign never matters to `abs`, so a negation, a select between
a value and its negation, and a multiply by another `abs` are looked
through; a sign extension narrows the `abs`; `abs (srem x, 2)` is `x & 1`.

## Example
```text
call i32 @llvm.abs.i32(i32 %n, i1 false)   ==>   call i32 @llvm.abs.i32(i32 %x, i1 false)
; %n = sub i32 0, %x
```

MIRROR: the order of the folds and each pattern.

DEVIATE 1: `getKnownSignOrZero` (`abs(x) -> x` where x >= 0, `-x` where
x < 0) is not translated; it needs its ValueTracking function first. Exit
2026-11-09.

DEVIATE 2: no vectors.

## Used by
`combine` (its `visit_call`, the `llvm.abs` intrinsic).
