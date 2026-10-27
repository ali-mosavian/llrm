> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/libmcalls.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/libmcalls.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/libmcalls.rs sha256:6cc4f3c46e464c40f3809fe97073bac45189fd06d3c73f1cd2c24d5513968d62 -->

# libmcalls

Turns the rounding intrinsics x87 has no instruction for into calls of the
C library: `llvm.floor.f64` is a call of `floor`.
Translated from the legalizer's libcall expansion (`ConvertNodeToLibcall`),
which the DAG path does not have for floating point yet.

## How it works
`floor`, `ceil`, `trunc`, `round`, `roundeven` and `nearbyint` of `f64`,
`f32` (`floorf`) and `f80` (`floorl`) are each a call of the routine of that
name in the `libcalls` given, with its convention and its symbol. A name the
list lacks leaves the intrinsic alone. The result keeps the callee's
function type.

## Example
`%a = call double @llvm.floor.f64(double %x)` becomes
`%a = call double @floor(double %x)`.

## Runs
Once per module in `driver::optimized`, after the pipeline: the optimizer
folds and combines the intrinsic, not the call.

## See also
LLVM's `RuntimeLibcalls` (FLOOR_F64, CEIL_F64 and the rest of the family).
