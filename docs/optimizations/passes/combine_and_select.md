> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_and_select.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_and_select.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_and_select.rs sha256:8d37d0c2109ca4c9a1e625af2592f67589fbbd06f2cdd97ff0ced1cd1cd0f83d -->

# combine_and_select

`visitAnd` after the constant-mask folds: the xor/not identities and the
folds of an `and` with a boolean extension, which become selects.

Translation of
[`visitAnd`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2475)
from `and(shl(zext X, Y), SignMask)` (:2756) to the sign-bit select
(:2883), skipping what other families own.

MIRROR: the C++ order, and `createSelectInstWithUnknownProfile` is a plain
new `select` (no `simplifySelect`), as `replace_with_new` builds it.

DEVIATE 1: no floating-point folds (`fabs` of a bitcast) and no vectors.

DEVIATE 2: `narrowMaskedBinOp`, `foldAndOrOfSelectUsingImpliedCond`,
`matchDeMorgansLaws`, `foldCastedBitwiseLogic`,
`foldBinopOfSextBoolToSelect` and the tail from
`sinkNotIntoOtherHandOfLogicalOp` on are the next strip.

DEVIATE 3: `getFreelyInverted` (the not of a value that is not a one-use
`xor`) is not translated: the two `(A ^ B) & ((B ^ C) ^ A)` folds apply
only when the other operand has one use, where C++ builds the not directly.

## How it works
Each fold matches the operands of the `and` and returns the new
instruction, or nothing; `fold_and_select` tries them in C++ order.

## Used by
`combine_and::visit_and`.

## Example
`sext(a) & b` is `a ? b : 0`:

```text
%s = sext i1 %a to i32
%r = and i32 %s, %b   ; %r = select i1 %a, i32 %b, i32 0
```
