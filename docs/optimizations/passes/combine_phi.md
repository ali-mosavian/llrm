> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_phi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_phi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_phi.rs sha256:a061bc7ad10edc7be5bfc5e81a6c129ac2d3b1a6033e0778f6011831bf56aaf0 -->

# combine_phi

`foldOpIntoPhi`: an instruction that reads a phi becomes a phi of the
instruction applied to each incoming value.

Translation of
[`InstCombinerImpl::foldOpIntoPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1943)
and `simplifyInstructionWithPHI` (:1859) at d1106deb71cc.

MIRROR: the phi must have one user, or only users identical to the
instruction; every other operand must be phi-translatable; each incoming
value either simplifies with the operation applied, or (for at most one, on
an unconditional, reachable, non-back edge) gets a copy of the operation at
the end of its block; the implied-condition constant of an `icmp` on a
branch's edge.

DEVIATE 1: the `icmp` of a `ucmp`/`scmp` intrinsic with a constant (the
first `WillFold` case) is not made: MIR has no `ucmp`/`scmp`.

DEVIATE 2: the implied-condition test takes the compare's predicate without
`samesign` (MIR compares do not carry it), so it can only find less.

DEVIATE 3: no debug-use rewrite (`replaceAllDbgUsesWith`): `replace_value`
moves debug records of the phi with its uses.

## Used by
The `visit*` of the families that fold an operation into a phi
(`foldBinOpIntoSelectOrPhi`, `visitICmpInst`, `visitCastInst`...).

## Example
```text
%p = phi i16 [ 1, %a ], [ 2, %b ]          %q = phi i16 [ 6, %a ], [ 7, %b ]
%r = add i16 %p, 5                   ==>   (uses of %r read %q)
```
