> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/memcpycall.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/memcpycall.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/memcpycall.rs sha256:d6ae012e479a504af191332fc05d4baef179b7c77928476837e569e60d0a1a02 -->

# memcpycall

Turns an `llvm.memcpy` the target should not copy inline into a call of
`memcpy`, as the pin's `EmitTargetCodeForMemcpy` leaves it: a call for a
length the target cannot move in a few loads and stores.
Translated from llvm/lib/Target/X86/X86SelectionDAGInfo.cpp at d1106deb71cc.

## How it works
A copy of a variable length, of a constant length over `INLINE_LIMIT`
(`getMaxInlineSizeThreshold`) or of pointers aligned under four bytes is
a call of `memcpy(dst, src, len)` in the ABI's default convention; the
rest stays for the selector. Only on a target whose runtime says it has
`memcpy` (`libcalls` of its `c.toml`): the selector's `rep movs` stays
for the others (DEVIATE I1, AM-F-1).

## Example
`call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)` becomes
`call ptr @memcpy(ptr %d, ptr %s, i32 %n)`; with a constant length of 16
and 4-byte alignment it is left alone.

## Runs
Once per module in `driver::optimized`, when the target names `memcpy`
among its libcalls.

## See also
LLVM's `X86SelectionDAGInfo::EmitTargetCodeForMemcpy`
([#L328](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86SelectionDAGInfo.cpp#L328)).
