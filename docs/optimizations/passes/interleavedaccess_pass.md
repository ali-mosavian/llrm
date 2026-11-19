> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/interleavedaccess_pass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/interleavedaccess_pass.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/interleavedaccess_pass.rs sha256:f9bce5af60cec9f822012f006e05fa0c57f0154d4bb2c75be2b43b676ae9b135 -->

# interleavedaccess_pass

LLVM's `InterleavedAccessPass` (InterleavedAccessPass.cpp at
`d1106deb71cc`), store half, with AArch64's `lowerInterleavedStore`: a store
of a one-use `shufflevector` that re-interleaves two vectors becomes
`llvm.aarch64.neon.stN` of the `N` sub-vectors and the address; a load
whose users are all de-interleaving shuffles becomes `llvm.aarch64.neon.ldN`
and one `extractvalue` per shuffle.

## Example
`store <32 x i8> (shufflevector <16 x i8> %a, <16 x i8> %b, <0, 16, 1, 17,
..>), ptr %p` becomes `call void @llvm.aarch64.neon.st2.v16i8.p0(<16 x i8>
%a', <16 x i8> %b', ptr %p)` with `%a'`, `%b'` the sequential sub-shuffles,
which fold to `%a` and `%b`.

## Runs
After SelectOptimize, before CodeGenPrepare, where the target
`selects_interleaved_accesses`.

## Limits
DEVIATE IAP-1 (owner llrm-w11-coalescer-join, exit 2026-11-24): a load
with extract or binary-operator users (`tryReplaceExtracts`,
`replaceBinOpShuffles`); a sub-vector over 128 bits (several stores);
pointer elements; every 64-bit factor-2 store (the pin keeps those with
element 0 first and no paired store nearby); every nontemporal factor-2
store (the pin keeps them at `-Os`).
