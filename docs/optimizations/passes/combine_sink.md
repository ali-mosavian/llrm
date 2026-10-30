> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_sink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_sink.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_sink.rs sha256:4b690066cc4bcd2fe797fcc6d7d206011a851a1e23abca9efc07c304ed257fc2 -->

# combine_sink

Sinking one instruction to the block that is its only user's, as
InstCombine's driver does after popping it. Translated from LLVM's
`tryToSinkInstruction` and the `getOptionalSinkBlockForInst` lambda
([`InstructionCombining.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5565)).

## How it works
`sink_block` names where `inst` could go: the one block every undroppable
user is in (a phi's user is the block its value comes from), when that block
is reachable, is not `inst`'s own, and is either a block whose only
predecessor is `inst`'s, or one that ends the function (no successors).
More than 32 undroppable users, or two blocks, name none.

`sinkable` says whether `inst` may move there: not a phi, landing pad,
terminator or alloca; not something that may throw or not return; not a
convergent call; nothing that writes memory; a load or a call that reads
memory only into a block whose only predecessor is its own, and only when
nothing after it in its block writes (an `llvm.assume` does not), unless it
is `!invariant.load`.

`sink` moves it before the first non-phi, non-landing-pad instruction of the
destination. The `llvm.assume`s that read it outside the destination
(`droppable_uses`) read `true` after: the driver adds them to its worklist.

```text
b0: %x = add %a, 1            b0: br %c, b1, b2
    br %c, b1, b2     ==>     b1: %x = add %a, 1
b1: ret %x                        ret %x
b2: ret 0                     b2: ret 0
```

MIRROR: every check and its order; the user scan; the position.

DEVIATE, with the reason: a compare of a runtime call's flags result is
never sunk (the flags do not survive to another block); an instruction that
writes memory is never sunk (`SoleWriteToDeadLocal` needs a capture analysis
llrm has not translated); `isDroppable` is the `llvm.assume` call (MIR has
no operand bundles, so no `dereferenceable` bundle keeps a sink away);
`isEHPad` is `landingpad` (MIR has no funclet pads); debug records are not
moved (MIR has none).

## Example
`%x` is read only in `b1`, whose only predecessor is `b0`:
```mir
define i32 @f(i32 %a, i1 %c) {
b0:
  %x = add i32 %a, 1
  br i1 %c, label %b1, label %b2

b1:
  ret i32 %x

b2:
  ret i32 0
}
```
After:
```mir
define i32 @f(i32 %a, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %x = add i32 %a, 1
  ret i32 %x

b2:
  ret i32 0
}
```

From unit test `a_value_read_in_one_successor_moves_there` in
`combine_sink_tests.rs`.

## Runs
Not a pass: called by the combine driver once it lands.

## See also
`sink` (the sinking pass, which weighs frequencies).
