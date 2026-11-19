> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_minmax.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_minmax.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_minmax.rs sha256:3402fde887e2f6bcb8cea96f44d1d61c24637c5208da95deb8f91f315e1ff8c8 -->

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
of the constant, and the `i1` fold (`and` for `umin` and `smax`, `or` for
`umax` and `smin`).

MIRROR (second slice, in the pin's order): `umin(x, 1)` is `zext (x != 0)`;
`umax(nuw_shl(x, C), x + 1)` and its `nuw_mul` form; the canonical clamp
`smax(smin(X, MinC), MaxC)`; `smin(smax(X, -1), 1)` is `scmp(X, 0)`;
`min/max (neg nsw X), (neg nsw Y)`; `min/max X, -X` is `abs`; a constant
right-hand side the range of the left decides.

MIRROR (`scmp`/`ucmp`): `foldCmpIntrinsicOfExtended` (both extends of one
type, or one and a constant that narrows losslessly), `scmp(X, 0)` of a
value in [-1, 1], and `scmp(nsw (sub X, Y), 0)`.

DEVIATE 1: the rest of the min/max case is not translated:
`foldMinimumOverTrailingOrLeadingZeroCount`, `xor` of a power of two,
`moveNotAfterMinMax`, `moveAddAfterMinMax`, `minmax (X & NegPow2C, ...)`,
`foldClampRangeOfTwo`, `matchSAddSubSat`, the `reassociate*` and
`factorizeMinMaxTree` folds and `foldIntrinsicUsingDistributiveLaws`. Exit:
the next slices of `visitCallInst`.

## Used by
`visit_call` (`combine_memintrinsics`).
