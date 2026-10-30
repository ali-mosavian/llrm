> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_select.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_select.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_select.rs sha256:40ef5f954c0afd55603657b445ffb142c0bd312f0a1877ce6489b29619089d11 -->

# combine_select

`FoldOpIntoSelect`: an instruction that reads a select becomes a select of
the instruction applied to each arm.

Translation of
[`InstCombinerImpl::FoldOpIntoSelect`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1793)
with `simplifyOperationIntoSelectOperand` (:1763) and
`foldOperationIntoSelectOperand` (:1787) at d1106deb71cc.

MIRROR: a select with one user (or any, if asked); not `i1`; at least one
arm where the instruction simplifies (the select's own arm, the value an
`icmp eq`/`ne` condition says an operand is, `zext` of the condition as
1/0); the other arm gets a copy of the instruction; the new select keeps the
select's `prof` and `unpredictable` metadata.

DEVIATE 1: the min/max-intrinsic guard is not made: the callers pass binary
operators, compares, casts and address computations, and MIR has no
`MinMaxIntrinsic` among them.

`SimplifySelectsFeedingBinaryOp` (:1356) and `foldBinOpSelectBinOp`
(:1907): a binary operator of selects becomes a select of the operator on
each arm, and `(select c, x, x op k1) op k2` a select of constants on `x
op`.

MIRROR: the three shapes (two selects on one condition, either operand a
select with one use), the arm of an inner select an outer condition
implies, `(c ? y : -n) + z` as `c ? y : z - n`; the new select takes the
operation's name and the old select's `prof` and `unpredictable`.

DEVIATE 2: the new select and operators carry the operation's fast-math
flags (`IRBuilder::FastMathFlagGuard`) only on a float type; vectors do not
exist in MIR.

## Used by
`foldBinOpIntoSelectOrPhi` and the `visit*` of the families that fold an
operation into a select.

## Example
```text
%s = select i1 %c, i16 3, i16 4          %r = select i1 %c, i16 8, i16 9
%r = add i16 %s, 5                 ==>   (the add folds into both arms)
```
