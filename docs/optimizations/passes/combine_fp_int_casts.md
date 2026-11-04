> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_fp_int_casts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_fp_int_casts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_fp_int_casts.rs sha256:6f3251f369fc5d4ee5f522848c5811e75fdb2ec1eedd9493b77ac828bdb9f37a -->

# combine_fp_int_casts

`foldFBinOpOfIntCasts`: a floating-point `fadd`, `fsub` or `fmul` of two
integer conversions, or of one and a whole-number constant, is one integer
operation and one conversion, if the conversions are exact and the integer
operation cannot overflow.

Translation of
[`foldFBinOpOfIntCasts`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1705)
and `foldFBinOpOfIntCastsFromSign` (L1545) at d1106deb71cc.

MIRROR: both orders of sign are tried, unsigned first; `uitofp nneg` and
`sitofp` of a non-negative are the same. A signed product also needs both
operands non-zero (no `-0.0`).

DEVIATE 1: an `x86_fp80` constant is held as a double, so it joins only if
that double is a whole number the integer type holds exactly.

DEVIATE 2: `half`, `bfloat` and `fp128` (pin precisions 11, 8 and 113) never
fold: MIR has no such types, only `float`, `double` and `x86_fp80`. Exit:
the fold takes the precision of each type MIR gains.

## Used by
`combine_fp::visit_fadd_or_fmul` and `visit_fsub`, at the pin's position
less the folds before it that are not translated.

## Example
```text
%x = sitofp i16 %a to x86_fp80       %s = add nsw i16 %a, %b
%y = sitofp i16 %b to x86_fp80  ==>  %r = sitofp i16 %s to x86_fp80
%r = fadd x86_fp80 %x, %y
```
