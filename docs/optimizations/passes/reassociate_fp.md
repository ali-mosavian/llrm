> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate_fp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate_fp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate_fp.rs sha256:6f8c067b43e0c4c542ce3aecadb30ef8eeff8ad2e5658cdf8c9af336324439b3 -->

# reassociate_fp

Floating constants for Reassociate: the few it makes and reads.

DEVIATE: a floating constant is held as the nearest `f32` or `f64`
(`floatfold`'s representation), so an `x86_fp80` constant is a double and
`ConstantFP::get(Ty, NumFound)` is exact only up to 2^53.

## Example
No honest example: this holds helpers, not a pass; the reassociate
pages show the folds that use them.

## Used by
`crate::reassociate_optimize`, `crate::reassociate_negate`.
