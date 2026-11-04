> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_casted_logic.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_casted_logic.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_casted_logic.rs sha256:37fe75b264dd2cd6829265171a1e99a3e0516fa51d8617926ea2e573f9f50659 -->

# combine_casted_logic

`foldCastedBitwiseLogic`: `and`, `or` and `xor` of two like casts is the
cast of the logic at the source type.

Translation of
[`foldCastedBitwiseLogic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L1821)
and `shouldOptimizeCast` (:1764).

MIRROR: the C++ order and its one-use conditions.

DEVIATE 1: the `(A >> BW-1)` with `zext(icmp)` fold is left out.

DEVIATE 2: ours runs it from `visitAnd` ahead of the folds of a bool
extension, where the C++ reaches it after `reassociateBooleanAndOr`.

## Used by
`combine_and_select::fold_and_select`, `combine_or::visit_or` and
`combine_xor::visit_xor`.

## Example
Three tests of a boolean, `or`ed, narrow to one `or` of `i1`:

```text
%a = sext i1 %x to i16
%b = sext i1 %y to i16          %o = or i1 %x, %y
%r = or i16 %a, %b      ==>     %r = sext i1 %o to i16
```
