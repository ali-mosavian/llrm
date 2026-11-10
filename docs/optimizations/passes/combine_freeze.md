> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_freeze.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_freeze.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_freeze.rs sha256:884faa941ef03be662abd558af6b3d463d9abfd5de611f670ae3a8a2839b35b2 -->

# combine_freeze

InstCombine's `freeze`: `InstCombinerImpl::visitFreeze`
([InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5402)).

## How it works
A freeze of a phi goes into the phi's incoming values; a freeze of an
instruction whose only possibly-poison operand is one value moves to that
operand (`pushFreezeToPreventPoisonFromPropagating`, L5210); a freeze of
`undef` or `poison` becomes the one constant its users prefer: all ones
for an `or`, true for a select condition with a constant arm, the common
constant of a phi, else zero.

## Example
```text
%f = freeze i32 %a              ==>   %x.fr = freeze i32 %x
; %a = add i32 %x, 1                  %a = add i32 %x.fr, 1
```

MIRROR: the order of the folds and each guard.

DEVIATE 1: `foldFreezeIntoRecurrence` (L5271) and `freezeOtherUses`
(L5339) are not translated. Exit 2026-11-12.

DEVIATE 2: no vectors, so `getFreezeVectorReplacement` and the shuffle
guard are not here.

## Used by
`combine` (its dispatch on `Opcode::Freeze`).
