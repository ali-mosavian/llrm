> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_minmax.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_minmax.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_minmax.rs sha256:5255a562be23f879165aed11031daec611ce01badbc61b3fe5e63bd377c9751d -->

# combine_minmax

InstCombine's min/max intrinsics: the narrowing folds of
`InstCombinerImpl::visitCallInst` for `umin`, `umax`, `smin` and `smax`
([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L2261)).

## How it works
A min or max of two operands extended from one type, or of one extended
operand and a constant that fits the narrow type, is the extension of the
narrow min or max. The unsigned intrinsics try `zext` first, then `sext`
(the pin falls through from its unsigned case into its signed one); the
signed ones only `sext`.

## Example
```text
umin.i33(zext i16 %x to i33, 9)  ==>  zext (umin.i16(%x, 9)) to i33
```

MIRROR: both folds, their one-use tests and the lossless-truncation test
of the constant.

DEVIATE 1: the rest of the min/max case is not translated (clamp forms,
`i1` operands, `neg`, `xor` of a power of two, `not`, nested min/max,
`cttz`/`ctlz` and the saturating folds). Exit: the next slice of
`visitCallInst`.

## Used by
`visit_call` (`combine_memintrinsics`).
