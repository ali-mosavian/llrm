> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_casted_logic.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_casted_logic.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_casted_logic.rs sha256:c3a16b235660e12ab17f5a92bc480dacf5a05a1ecbf314eca1510451eb9d428a -->

# combine_casted_logic

`foldCastedBitwiseLogic`: `and`, `or` and `xor` of two like casts is the
cast of the logic at the source type.

Translation of
[`foldCastedBitwiseLogic`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L1821)
and `shouldOptimizeCast` (:1764).

MIRROR: the C++ order and its one-use conditions.

DEVIATE 1: `foldLogicCastConstant` (a cast and a constant) and the
`(A >> BW-1)` with `zext(icmp)` fold are left out.

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
