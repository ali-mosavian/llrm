> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/expandmemcmp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/expandmemcmp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/expandmemcmp.rs sha256:5b92d93636b3561640abd1b5866a9f3e28f29e245609ed2b0739c972baf94f05 -->

# expandmemcmp

A `memcmp` or `bcmp` of a constant size becomes loads and compares, as
LLVM's `ExpandMemCmp`.

## How it works
A translation of LLVM
[`ExpandMemCmp.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp)
at d1106deb71cc. The target's `enable_mem_cmp_expansion` names the load
sizes, the most loads and the loads in a block; the size is cut into loads
(greedy, or with one overlapping load at the end) and each block loads both
sides and compares. A call used only against zero is `xor`/`or` of the load
pairs; any other is packed `bswap`ped loads compared unsigned, in a
chain of blocks with an early exit to `res_block`. A call whose only user
asks for one outcome (`< 0`, `> 0`, ...) is one unsigned compare. Not in
`minsize`, nor in a function that is sanitized.

## Example
```text
%c = call i32 @memcmp(ptr %a, ptr %b, i64 8)      ; x86-64, %c == 0
%e = icmp eq i32 %c, 0
```
becomes `load i64` of both, `icmp ne`, `zext` to `i32`, and the `icmp eq`.

## Deviations
- DEVIATE E1 (exit 2026-12-31, pin permalink [L778](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L778)):
  the pin's `int` is 32 bits. Here it is the call's own type, and a two-byte
  compare in a 16-bit `int` cannot be a subtraction (it wraps), so it takes
  the `ucmp` path. Exit: a pin with a 16-bit `int`.
- DEVIATE E2 (exit 2026-12-31, pin permalink [L414](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L414)):
  a load at an offset from a constant global is not folded here (the pin's
  offset is a constant expression); the `getelementptr` index has the
  pointer's index width, not `i64`.
- DEVIATE E3 (exit 2026-12-31, pin permalink [L1061](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L1061)):
  `SimplifyInstructionsInBlock` is not run; the combine that follows cleans
  the expansion. Dominators are not kept (no `DomTreeUpdater`).
- DEVIATE E4: the profile-guided `shouldOptimizeForSize` is false (no
  profile here), and `setExplicitlyUnknownBranchWeightsIfProfiled` has no
  profile to read.

## Parity slot
`expandmemcmp@1673` (PassBuilderPipelines.cpp#L1673), between
`divrempairs` and `tailrec`.

## Used by
Named by `pipeline`.
