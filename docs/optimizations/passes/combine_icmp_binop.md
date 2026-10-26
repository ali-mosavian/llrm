> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_icmp_binop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_icmp_binop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_icmp_binop.rs sha256:de59b277e0c8f1f30a7dcd9b65618ae3343acb4ca718dc466db972b4af596e4e -->

# combine_icmp_binop

InstCombine's `foldICmpBinOp`: the compare of a binary operation, from
[`InstCombineCompares.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L5301)
at d1106deb71cc.

MIRROR: the order of the steps and each condition (the `nsw`/`nuw` rule of
`hasNoWrapProblem` included), from the function's start to the end of its
`sub` section (`icmp (0-X) < C`).

The steps after that stay in their place as `// OWED:` comments, in
LLVM's order: `foldICmpXorXX`, `foldICmpOrXX`, the shared multiplier
(`X * Z pred Y * Z`), the `srem` against its divisor, the two operations of
one opcode with a shared right operand (`a+x`, `a^x`, `a*C`, the exact
divisions and shifts), `foldICmpAndXX`, `foldMultiplicationOverflowCheck`,
`foldICmpWithTruncSignExtendedVal` and
`foldShiftIntoShiftInAnotherHandOfAndInICmp`; and the first step,
`foldICmpXNegX`.

DEVIATE 1 (permanent): no vectors, no `ConstantExpr`: `m_APInt`,
`m_ImmConstant` and `m_SignMask` read scalar integer constants.

## Used by
[`crate::combine_compares::visit_icmp`](combine_compares.md), between the `x >u INT_MAX` fold
and `foldICmpInstWithConstant`.

## How it works
An operand that is an `add` (or an `or disjoint`) or a `sub` is split into
its parts. A compare of `A+B` against `A`, of two sums that share a term,
of two differences that share one, or against a constant that differs from
another by a step is the compare of what is left, where the operation
cannot wrap in the compare's signedness. A relational compare against
`C + -1` or `C + 1` (the step) swaps strict for non-strict, where
the other side is a multiple of the step.

## Example
`a > b + -1` (`nsw`) is `a >= b`; with `b < a` known, the branch on it
folds in `SimplifyCFG`:

```text
%d = add nsw i16 %b, -1
%g = icmp sgt i16 %a, %d   ; %g = icmp sge i16 %a, %b
```
