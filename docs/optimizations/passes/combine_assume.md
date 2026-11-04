> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_assume.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_assume.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_assume.rs sha256:fe53f498af3f402fed04e87907b18c4f7242da089670c7d703befb2439bacba3 -->

# combine_assume

InstCombine's `llvm.assume`: the condition folds of
`InstCombinerImpl::visitCallInst`
([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L3837)).

## How it works
`assume(a && b)` is `assume(a); assume(b)`, and `assume(!(a || b))` is
`assume(!a); assume(!b)`: each condition then reaches ValueTracking alone.
An assume of a constant true is erased; of false, poison or undef it is
unreachable code, which becomes a store of `true` to a poison pointer
(`CreateNonTerminatorUnreachable`) before the assume is erased.

## Example
```text
call void @llvm.assume(i1 %ab)      ==>   call void @llvm.assume(i1 %a)
; %ab = and i1 %a, %b                     call void @llvm.assume(i1 %b)
```

MIRROR: the order of the folds and each pattern.

DEVIATE 1: MIR has no operand bundles (#2203), so the bundle loop, the
`nonnull` and `align` bundles made from an `icmp ne ptr, null` or an
alignment mask, and the `hasOperandBundles` early exit are not translated.
DEVIATE 2: no `AssumptionCache::updateAffectedValues`; the cache is built
from the function when asked.

## Used by
`combine` (its `visit_call`, the `llvm.assume` intrinsic).
