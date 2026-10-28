> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_muldivrem.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_muldivrem.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_muldivrem.rs sha256:a0e73af13b59c1b849e2814308f3ad05093a8fe8b2cb9c36cb026d3b3d3e53e8 -->

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

DEVIATE 2 (temporary): steps that call a helper another family owns are
left out and named where they stood: `foldBinopWithPhiOperands` (phi
family),
`foldBinOpIntoSelectOrPhi`, `foldBinOpSelectBinOp`,
`foldBinOpOfSelectAndCastOfSelectCondition` and `FoldOpIntoSelect` (select
family). Each is a
`// DEVIATE 2:` comment at its place in the order.

DEVIATE 3 (temporary): the folds that build an intrinsic call (`abs`,
`umin`, `umax`, `cttz`) are left out until the Builder can declare an
intrinsic (`combine_build` DEVIATE 2); each is a `// DEVIATE 3:` comment at
its place. `takeLog2` cannot look through `umin`/`umax` for the same reason.

DEVIATE 4: a constant that `ConstantExpr::getNeg` or `getShl` would leave
as an expression is not built (MIR holds no binary constant expressions):
the fold is skipped where the constants do not fold to a number.

DEVIATE 5 (temporary): `narrowMathIfNoOverflow` (`InstructionCombining.cpp`,
shared with `visitAdd`) is left out until `combine_shared` has it.

DEVIATE 6 (temporary): `Negator::Negate` is not translated; it is taken to
fail, so the two folds that use it are skipped and the step after each
(the `zext`/`sext` multiply by `-1 << C`) is tried where C++ would stop.

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
