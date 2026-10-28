> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_xor.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_xor.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_xor.rs sha256:494be3741b8f08e256f3bfe0156191b31fcf1c329f2b19abfbb050e9e44004fa -->

# combine_xor

`InstCombinerImpl::visitXor`: the integer `xor` folds.

Translation of
[`visitXor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L5458)
at d1106deb71cc, the folds that need only the operand patterns, in the
order it reaches them (`FOLDS`: a new one is a function and an entry).

MIRROR: the C++ order, which operand of a commutative match is bound
first, and the one-use conditions.

DEVIATE 1: no vectors, and only integer constants (`ConstantExpr` and
`undef` lanes are not matched or made).

DEVIATE 2: the folds that call a helper not yet translated are left out,
each named where it stood: `foldXorToXor`, `visitMaskedMerge`, the `ctlz`/`cttz` and
`fneg` bitcast folds, the `i1` select form of `(A & B) ^ (A | C)`,
`foldXorOfICmps`, `foldCastedBitwiseLogic`, `canonicalizeAbs`,
`reassociateForUses`, `canonicalizeLogicFirst`, `foldLogicOfIsFPClass`,
`canonicalizeConditionalNegationViaMathToSelect`,
`foldBinOpOfDisplacedShifts`, `foldBitwiseLogicWithIntrinsics` and
`foldMaskedAddXorPattern`.

DEVIATE 3: `foldNot` makes its `not (cmp A, B)` arm for an integer compare
with one use. Owed, in its order: DeMorgan with an inverted operand, the
binary-operator arms (`~(X + C)`, `~(X - Y)`, shifts), the `bitcast` arms,
`sinkNotIntoLogicalOp`, min/max and `is_fpclass`, the select of compares,
`foldNotXor`, `getFreelyInverted`, a `not` of a compare with several uses
(`canFreelyInvertAllUsersOf`, now inline in `combine_compares`), and `fcmp`.

## Example
`Y ^ (X | Y)` is `X & ~Y`:

```text
%o = or i32 %x, %y
%r = xor i32 %y, %o   ==>   %n = xor i32 %y, -1 ; %r = and i32 %x, %n
```

## Used by
Named by `combine`.
