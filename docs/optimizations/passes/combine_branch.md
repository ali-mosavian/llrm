> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_branch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_branch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_branch.rs sha256:8819f73cbf9866db0184c3d6b7d82289d485b7b30aad77f97c2ee196c3322f1f -->

# combine_branch

`visitCondBrInst`: the canonical forms of a conditional branch.

Translation of
[`InstCombinerImpl::visitCondBrInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L4347)
at d1106deb71cc. `visit` has already asked `simplifyInstruction`.

MIRROR: `br (not X), T, F` becomes `br X, F, T`, so that the guards SCEV
and the loop passes read are plain compares; a branch to one block on both
edges reads no condition.

MIRROR: a branch that reaches the end of the visit is registered with the
`DomConditionCache` (`DC.registerBranch`), so a query sees the branches
visited before it and no others.

DEVIATE 1 (temporary): not made: the logical-and-with-invert form (MIR has
no logical `select` pattern here yet), the `fcmp` predicate canonical form
(`isCanonicalPredicate`), and the constant condition at visit time (the
driver folds a branch on a number, and `combine::prepare` kills the edges
of an undef or poison one, `combine.rs`).

MIRROR: every use of the condition that an edge dominates becomes `true`
(the first edge) or `false` (the second): a second branch on the same
compare, further down, reads a number.

DEVIATE 2: branch weights are not swapped (MIR carries none on a branch).

## Example
```text
%n = xor i1 %c, true                  br i1 %c, label %f, label %t
br i1 %n, label %t, label %f
```
