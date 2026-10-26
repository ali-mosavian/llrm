> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_distribute.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_distribute.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_distribute.rs sha256:1a3163391cdf62b5c7a10d501d94a558f7991292ec86dd89da50e2080ac07115 -->

# combine_distribute

`InstCombinerImpl::foldUsingDistributiveLaws`: factor a common term out of
two operands of a binary operator, or expand an operator over an operand
it distributes over when both halves simplify.

Translation of
[`foldUsingDistributiveLaws`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1200),
[`tryFactorizationFolds`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1157),
[`tryFactorization`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L750)
and the helpers they read (`leftDistributesOverRight`,
`rightDistributesOverLeft`, `getIdentityValue`,
`getBinOpsForFactorization`) in the order the C++ reaches them, ending in
`SimplifySelectsFeedingBinaryOp`.

MIRROR: factor first (`(A op' B) op (A op' D)` to `A op' (B op D)`, and the
`(A op' B) op C` forms with the identity of `op'` for the missing arm, a
`shl` by a constant read as the `mul` of `1 << C` under `add` and `sub`),
then expand `(A op' B) op C` and `A op (B op' C)` when both halves
simplify (without `undef`) or one half is the identity of `op'`.

DEVIATE 1: `getBinOpsForFactorization` keeps the `shl` as it is where
`1 << C` does not fold to a constant (the C++ asserts it does).

DEVIATE 2: the `nsw`/`nuw` of a factored `add` of `mul`s are set on the
result only when `tryFactorization` made it; the C++ also sets them on an
existing binary operator the builder's folder returned.

## Used by
`visit_and`, `visit_or`, `visit_xor`, `visit_add`, `visit_sub`, `visit_mul`.

## Example
```text
%a = mul i32 %x, %y
%b = mul i32 %x, %z                ==>   %s = add i32 %y, %z
%r = add i32 %a, %b                      %r = mul i32 %x, %s
```
