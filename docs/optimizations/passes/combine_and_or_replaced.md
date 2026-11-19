> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_and_or_replaced.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_and_or_replaced.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_and_or_replaced.rs sha256:c0d373daec510ebb4e9824a18b069774d6dfdae2b203e1ed62c7eee8e305418f -->

# combine_and_or_replaced

`simplifyAndOrWithOpReplaced`: in `X | Y` try `0` for `Y` inside `X`, in
`X & Y` try `-1`, looking only through bitwise operations.

Translation of
[`simplifyAndOrWithOpReplaced`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2371)
at d1106deb71cc and of its two callers, the tails of `visitAnd` (:2959)
and `visitOr` (:4775).

MIRROR: depth limit 3, the one-use test that turns the rewrite into a
simplification only, and `simplifyBinOp` before a new instruction.

DEVIATE 1: the callers run after the other translated folds of the visit
(the folds between them and `foldBitwiseLogicWithIntrinsics` are not all
translated, and those that are sit in a different order here).

## Example
`(x ^ 7) | x` reads `x` as 0 inside the xor, which is 7:

```text
%a = xor i8 %x, 7
%r = or i8 %a, %x      ==>      %r = or i8 %x, 7
```

## Used by
Named by `combine_and`, `combine_or`.
