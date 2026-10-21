> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_branch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_branch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_branch.rs sha256:265889fb905d2487a375256f88b1f4cb263dc0260b17820b2e945135a5de69a5 -->

# combine_branch

`visitCondBrInst`: the canonical forms of a conditional branch.

Translation of
[`InstCombinerImpl::visitCondBrInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L4347)
at d1106deb71cc. `visit` has already asked `simplifyInstruction`.

MIRROR: `br (not X), T, F` becomes `br X, F, T`, so that the guards SCEV
and the loop passes read are plain compares; a branch to one block on both
edges reads no condition.

DEVIATE 1 (temporary): not made: the logical-and-with-invert form (MIR has
no logical `select` pattern here yet), the `fcmp` predicate canonical form
(`isCanonicalPredicate`), the undef and constant conditions (the driver
folds a branch on a number, `combine.rs`), and the replacement of the uses
of the condition dominated by an edge with true or false.

DEVIATE 2: branch weights are not swapped (MIR carries none on a branch).

## Example
```text
%n = xor i1 %c, true                  br i1 %c, label %f, label %t
br i1 %n, label %t, label %f
```
