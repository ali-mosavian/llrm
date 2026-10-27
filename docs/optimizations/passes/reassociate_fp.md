> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_fp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_fp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_fp.rs sha256:cb93cd729a029a54216b0e81f505f5fb0ad494f95de9024f95f3ef4594049a54 -->

# reassociate_fp

Floating constants for Reassociate: the few it makes and reads.

DEVIATE: a floating constant is held as the nearest `f32` or `f64`
(`floatfold`'s representation), so an `x86_fp80` constant is a double and
`ConstantFP::get(Ty, NumFound)` is exact only up to 2^53.

## Used by
`crate::reassociate_optimize`, `crate::reassociate_negate`.

## Example
`x * 4.0 * 0.25`: the constants fold to `1.0` (`fold_float`), the identity
of `fmul`, so the result is `x`.
