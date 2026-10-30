> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_muldivrem.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_muldivrem.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_muldivrem.rs sha256:173d3a7aec65d613a3e029a639be05385a21ef2c133393dbb6274370cb369033 -->

# combine_muldivrem

InstCombine's `visitMul`, `visitUDiv`, `visitSDiv`, `visitURem`, `visitSRem`
and the helpers they call, from
[`InstCombineMulDivRem.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp)
at d1106deb71cc.

`visit_mul` (and likewise `visit_udiv`, `visit_sdiv`, `visit_urem`,
`visit_srem`) is
[`visitMul`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L193):
the same steps in the same order, each a function named for the C++ one
(or for the block of the `visit*` it is) with its own permalink. A step that
returns `nullptr` there returns `None` here, one that returns an
instruction returns `Some(Visit)` (a new instruction replaces the visited
one; `&I` is `Visit::Changed`).

MIRROR: the order of the steps and each condition. A `m_c_*` match commits
to the first operand order that matches, as in C++.

DEVIATE 1: `foldVectorBinop` is not translated: MIR has no vector
operations, so no `ConstantVector` case either.

DEVIATE 2 (temporary): `foldBinOpSelectBinOp` and
`foldBinOpOfSelectAndCastOfSelectCondition` (select family) are left out,
each a `// DEVIATE 2:` comment at its place in the order.

DEVIATE 4: a constant that `ConstantExpr::getNeg` or `getShl` would leave
as an expression is not built (MIR holds no binary constant expressions):
the fold is skipped where the constants do not fold to a number.

DEVIATE 8: `simplifyICmpInst` in `visitURem` is asked without the context
instruction, dominator tree and assumptions (no query that takes them):
it can only find less.

## Used by
`combine::visit`, once the mul, div and rem slots are flipped.

## How it works
Each `visit_*` returns the first step that changes the multiply. The steps
rewrite it to a cheaper form (`x * 2^k` to a shift, `x * -1` to a negation,
a multiply by a boolean to a `select`), or replace it by an existing value.
A rewrite that needs new operands builds them with the `Combiner`'s
builder, which folds through `simplify*` first. If nothing rewrites it,
known bits and ranges may add a multiply's `nsw` and `nuw`. A divide by a
power of two becomes a shift, one by a constant is folded into its
operands, and `srem`/`urem` into a mask or the other remainder.

## Example
`(zext i1 %b) * %y` is a select:

```text
%z = zext i1 %b to i32
%r = mul i32 %z, %y   ; %r = select i1 %b, i32 %y, i32 0
```
