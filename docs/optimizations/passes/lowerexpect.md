> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lowerexpect.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lowerexpect.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lowerexpect.rs sha256:4a557486b6e7fadaf08e7002a50ac2409f8b7b36cf32bc70294e983497476bfb -->

# lowerexpect

LLVM's `LowerExpectIntrinsic`: `llvm.expect` becomes branch weights and
goes.

A translation of
[`LowerExpectIntrinsic.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/LowerExpectIntrinsic.cpp)
at d1106deb71cc: a `br`, `switch` or `select` whose condition is an expect
call (or an `==`/`!=` of one against a constant) gets `!prof` weights, 2000
for the expected outcome and 1 for the others; the phi that defines the
value an expect call tests marks the branch that feeds it a constant it
does not expect; each call is replaced by its first argument.

## How it works
`lower` visits the blocks in layout order, as LLVM's pass does: the block's
terminator first (`handleBrSelExpect`, `handleSwitchExpect`), then its
instructions backwards (a select before the call it tests is removed, a
phi definition before its call is erased).

## Deviations
- `llvm.expect.with.probability` is not in MIR, so `getBranchWeight`'s
  probability arm, `Confidence` and `BranchCount - 1` are not translated.
- `misexpect::checkExpectAnnotations` and `checkFrontendInstrumentation`
  (profile-guided diagnostics) are not translated: llrm has no profile data.
- `setBranchWeights` states `!prof` `branch_weights` without the `expected`
  origin operand: no pass here reads it.

## Example
```mir
%e = call i32 @llvm.expect.i32(i32 %x, i32 0)
%c = icmp ne i32 %e, 0
br i1 %c, label %t, label %f
```
becomes `%c = icmp ne i32 %x, 0` and `br i1 %c, ..., !prof
!{!"branch_weights", i32 1, i32 2000}`: the false edge is the likely one.

## Used by
`pipeline` (`Phase::Early`, the first slot, as `EarlyFPM.addPass` at
PassBuilderPipelines.cpp:1177).
